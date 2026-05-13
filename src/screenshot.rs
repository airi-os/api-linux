use crate::diagnostics::hydrate_session_bus_env;
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use schemars::JsonSchema;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zbus::{
    message::{Message, Type as MessageType},
    zvariant::{OwnedObjectPath, OwnedValue, Value},
    MatchRule, MessageStream, Proxy,
};

const PORTAL_REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const PORTAL_REQUEST_PATH_NAMESPACE: &str = "/org/freedesktop/portal/desktop/request";

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ScreenshotCapture {
    pub mime_type: String,
    pub data_url: String,
    pub source: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<ScreenshotArea>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ScreenshotArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScreenshotCleanup {
    DeletePath(PathBuf),
    Preserve,
}

pub async fn capture_screenshot() -> Result<ScreenshotCapture> {
    hydrate_session_bus_env();

    match capture_with_gnome_shell().await {
        Ok(capture) => Ok(capture),
        Err(gnome_error) => match capture_with_portal().await {
            Ok(capture) => Ok(capture),
            Err(portal_error) => Err(anyhow!(
                "GNOME Shell screenshot failed: {gnome_error}; XDG portal screenshot failed: {portal_error}"
            )),
        },
    }
}

pub async fn capture_screenshot_area(area: ScreenshotArea) -> Result<ScreenshotCapture> {
    hydrate_session_bus_env();
    match capture_with_gnome_shell_area(area).await {
        Ok(capture) => Ok(capture),
        Err(area_error) => {
            let full_capture = capture_screenshot().await.with_context(|| {
                format!(
                    "GNOME Shell area screenshot failed: {area_error:#}; full-screen fallback failed"
                )
            })?;
            crop_capture(full_capture, area).with_context(|| {
                format!(
                    "GNOME Shell area screenshot failed: {area_error:#}; full-screen fallback could not be cropped"
                )
            })
        }
    }
}

async fn capture_with_gnome_shell() -> Result<ScreenshotCapture> {
    let connection = zbus::Connection::session()
        .await
        .context("failed to connect to session bus")?;
    let proxy = Proxy::new(
        &connection,
        "org.gnome.Shell.Screenshot",
        "/org/gnome/Shell/Screenshot",
        "org.gnome.Shell.Screenshot",
    )
    .await
    .context("failed to create GNOME Shell screenshot proxy")?;
    let path = temp_png_path("gnome-shell");
    let filename = path
        .to_str()
        .context("temporary screenshot path is not valid UTF-8")?;
    let result = proxy.call("Screenshot", &(false, false, filename)).await;
    let (success, filename_used): (bool, String) = match result {
        Ok(result) => result,
        Err(error) => {
            cleanup_gnome_requested_path(&path);
            return Err(error).context("GNOME Shell Screenshot call failed");
        }
    };

    if !success {
        cleanup_gnome_requested_path(&path);
        bail!("GNOME Shell reported screenshot failure");
    }

    read_png_as_capture(
        PathBuf::from(filename_used),
        "gnome-shell",
        ScreenshotCleanup::DeletePath(path),
        None,
    )
    .await
}

async fn capture_with_gnome_shell_area(area: ScreenshotArea) -> Result<ScreenshotCapture> {
    area.validate()?;

    let connection = zbus::Connection::session()
        .await
        .context("failed to connect to session bus")?;
    let proxy = Proxy::new(
        &connection,
        "org.gnome.Shell.Screenshot",
        "/org/gnome/Shell/Screenshot",
        "org.gnome.Shell.Screenshot",
    )
    .await
    .context("failed to create GNOME Shell screenshot proxy")?;
    let path = temp_png_path("gnome-shell-area");
    let filename = path
        .to_str()
        .context("temporary screenshot path is not valid UTF-8")?;
    let width = area.width_i32()?;
    let height = area.height_i32()?;
    let result = proxy
        .call(
            "ScreenshotArea",
            &(area.x, area.y, width, height, false, filename),
        )
        .await;
    let (success, filename_used): (bool, String) = match result {
        Ok(result) => result,
        Err(error) => {
            cleanup_gnome_requested_path(&path);
            return Err(error).context("GNOME Shell ScreenshotArea call failed");
        }
    };

    if !success {
        cleanup_gnome_requested_path(&path);
        bail!("GNOME Shell reported ScreenshotArea failure");
    }

    read_png_as_capture(
        PathBuf::from(filename_used),
        "gnome-shell-area",
        ScreenshotCleanup::DeletePath(path),
        Some(area),
    )
    .await
}

async fn capture_with_portal() -> Result<ScreenshotCapture> {
    let connection = zbus::Connection::session()
        .await
        .context("failed to connect to session bus")?;
    let token = request_token();
    // Some portals rewrite the request handle, so subscribe before calling Screenshot
    // and filter by the returned handle instead of subscribing after the call.
    let mut response_stream = portal_response_stream(&connection).await?;

    let portal_proxy = Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot",
    )
    .await
    .context("failed to create XDG portal screenshot proxy")?;
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("interactive", Value::from(false));
    let handle: OwnedObjectPath = portal_proxy
        .call("Screenshot", &("", options))
        .await
        .context("XDG portal Screenshot call failed")?;

    let (response_code, results) = tokio::time::timeout(
        Duration::from_secs(20),
        wait_for_portal_response(&mut response_stream, handle.as_str()),
    )
    .await
    .context("timed out waiting for XDG portal screenshot response")??;

    if response_code != 0 {
        bail!("XDG portal screenshot was denied or cancelled with response code {response_code}");
    }

    let uri_value = results
        .get("uri")
        .context("XDG portal screenshot response did not include a uri")?;
    let uri: String = uri_value
        .try_clone()
        .context("failed to clone XDG portal screenshot uri")?
        .try_into()
        .context("XDG portal screenshot uri was not a string")?;
    let path = file_uri_to_path(&uri)?;

    read_png_as_capture(
        path,
        "xdg-desktop-portal",
        ScreenshotCleanup::Preserve,
        None,
    )
    .await
}

async fn portal_response_stream(connection: &zbus::Connection) -> Result<MessageStream> {
    let response_rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .interface(PORTAL_REQUEST_INTERFACE)?
        .member("Response")?
        .path_namespace(PORTAL_REQUEST_PATH_NAMESPACE)?
        .build();

    MessageStream::for_match_rule(response_rule, connection, None)
        .await
        .context("failed to subscribe to XDG portal screenshot responses")
}

async fn wait_for_portal_response(
    response_stream: &mut MessageStream,
    request_path: &str,
) -> Result<(u32, HashMap<String, OwnedValue>)> {
    loop {
        let response = response_stream
            .next()
            .await
            .context("XDG portal screenshot response stream ended")?
            .context("XDG portal screenshot response stream failed")?;

        if !portal_response_matches_path(&response, request_path) {
            continue;
        }

        return response
            .body()
            .deserialize()
            .context("failed to decode XDG portal screenshot response");
    }
}

fn portal_response_matches_path(response: &Message, request_path: &str) -> bool {
    response
        .header()
        .path()
        .is_some_and(|path| path.as_str() == request_path)
}

async fn read_png_as_capture(
    path: PathBuf,
    source: &str,
    cleanup: ScreenshotCleanup,
    region: Option<ScreenshotArea>,
) -> Result<ScreenshotCapture> {
    let result = read_png_as_capture_inner(&path, source, region);
    if let ScreenshotCleanup::DeletePath(path) = cleanup {
        let _ = fs::remove_file(path);
    }
    result
}

fn read_png_as_capture_inner(
    path: &Path,
    source: &str,
    region: Option<ScreenshotArea>,
) -> Result<ScreenshotCapture> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read screenshot file {}", path.display()))?;
    if bytes.is_empty() {
        bail!("screenshot file was empty: {}", path.display());
    }
    let (width, height) = png_dimensions(&bytes)?;
    let encoded = STANDARD.encode(bytes);
    Ok(ScreenshotCapture {
        mime_type: "image/png".to_string(),
        data_url: format!("data:image/png;base64,{encoded}"),
        source: source.to_string(),
        width,
        height,
        region,
    })
}

fn crop_capture(capture: ScreenshotCapture, area: ScreenshotArea) -> Result<ScreenshotCapture> {
    let bytes = png_bytes_from_data_url(&capture.data_url)?;
    let (cropped_bytes, clipped_area) = crop_png_bytes(&bytes, area)?;
    let (width, height) = png_dimensions(&cropped_bytes)?;
    let encoded = STANDARD.encode(cropped_bytes);
    Ok(ScreenshotCapture {
        mime_type: "image/png".to_string(),
        data_url: format!("data:image/png;base64,{encoded}"),
        source: format!("{}-crop", capture.source),
        width,
        height,
        region: Some(clipped_area),
    })
}

fn png_bytes_from_data_url(data_url: &str) -> Result<Vec<u8>> {
    let payload = data_url
        .strip_prefix("data:image/png;base64,")
        .context("screenshot data URL was not an image/png base64 payload")?;
    STANDARD
        .decode(payload)
        .context("failed to decode screenshot data URL")
}

fn crop_png_bytes(bytes: &[u8], area: ScreenshotArea) -> Result<(Vec<u8>, ScreenshotArea)> {
    area.validate()?;

    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .context("failed to decode screenshot PNG header")?;
    let mut decoded = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut decoded)
        .context("failed to decode screenshot PNG pixels")?;
    if info.bit_depth != png::BitDepth::Eight {
        bail!(
            "unsupported screenshot PNG bit depth after decoding: {:?}",
            info.bit_depth
        );
    }
    let channels = channels_for_color_type(info.color_type)?;
    let decoded = &decoded[..info.buffer_size()];
    let clipped = clip_area_to_image(area, info.width, info.height)?;
    let image_width = usize::try_from(info.width).context("screenshot width is too large")?;
    let x = usize::try_from(clipped.x).context("clipped screenshot x is negative")?;
    let y = usize::try_from(clipped.y).context("clipped screenshot y is negative")?;
    let width = usize::try_from(clipped.width).context("clipped screenshot width is too large")?;
    let height =
        usize::try_from(clipped.height).context("clipped screenshot height is too large")?;
    let row_len = width
        .checked_mul(channels)
        .context("clipped screenshot row is too wide")?;
    let mut cropped = Vec::with_capacity(
        row_len
            .checked_mul(height)
            .context("clipped screenshot is too large")?,
    );

    for row in 0..height {
        let row_index = y
            .checked_add(row)
            .context("clipped screenshot row overflowed")?;
        let source_start = row_index
            .checked_mul(image_width)
            .and_then(|offset| offset.checked_add(x))
            .and_then(|offset| offset.checked_mul(channels))
            .context("clipped screenshot source offset overflowed")?;
        let source_end = source_start
            .checked_add(row_len)
            .context("clipped screenshot source end overflowed")?;
        cropped.extend_from_slice(&decoded[source_start..source_end]);
    }

    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, clipped.width, clipped.height);
        encoder.set_color(info.color_type);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .context("failed to encode cropped screenshot PNG header")?;
        writer
            .write_image_data(&cropped)
            .context("failed to encode cropped screenshot PNG pixels")?;
    }

    Ok((encoded, clipped))
}

fn channels_for_color_type(color_type: png::ColorType) -> Result<usize> {
    match color_type {
        png::ColorType::Grayscale => Ok(1),
        png::ColorType::Rgb => Ok(3),
        png::ColorType::Indexed => bail!("indexed screenshot PNG did not expand to RGB"),
        png::ColorType::GrayscaleAlpha => Ok(2),
        png::ColorType::Rgba => Ok(4),
    }
}

fn clip_area_to_image(
    area: ScreenshotArea,
    image_width: u32,
    image_height: u32,
) -> Result<ScreenshotArea> {
    let x0 = i64::from(area.x).max(0);
    let y0 = i64::from(area.y).max(0);
    let x1 = i64::from(area.x)
        .checked_add(i64::from(area.width))
        .context("screenshot area x coordinate overflowed")?
        .min(i64::from(image_width));
    let y1 = i64::from(area.y)
        .checked_add(i64::from(area.height))
        .context("screenshot area y coordinate overflowed")?
        .min(i64::from(image_height));

    if x1 <= x0 || y1 <= y0 {
        bail!(
            "screenshot area x={}, y={}, width={}, height={} is outside full screenshot {}x{}",
            area.x,
            area.y,
            area.width,
            area.height,
            image_width,
            image_height
        );
    }

    Ok(ScreenshotArea {
        x: x0.try_into().context("clipped screenshot x is too large")?,
        y: y0.try_into().context("clipped screenshot y is too large")?,
        width: (x1 - x0)
            .try_into()
            .context("clipped screenshot width is too large")?,
        height: (y1 - y0)
            .try_into()
            .context("clipped screenshot height is too large")?,
    })
}

fn cleanup_gnome_requested_path(path: &Path) {
    let _ = fs::remove_file(path);
}

fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        bail!("screenshot file was not a valid PNG");
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if width == 0 || height == 0 {
        bail!("screenshot PNG had invalid dimensions {width}x{height}");
    }
    Ok((width, height))
}

fn file_uri_to_path(uri: &str) -> Result<PathBuf> {
    let Some(rest) = uri.strip_prefix("file://") else {
        bail!("unsupported screenshot uri: {uri}");
    };
    Ok(PathBuf::from(percent_decode(rest)))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    decoded.push(byte);
                    index += 3;
                    continue;
                }
            }
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn temp_png_path(source: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "computer-use-linux-{source}-{}.png",
        unique_suffix()
    ))
}

fn request_token() -> String {
    format!("computer_use_linux_{}", unique_suffix().replace('-', "_"))
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

impl ScreenshotArea {
    pub fn validate(self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            bail!(
                "screenshot area must have positive width and height, got {}x{}",
                self.width,
                self.height
            );
        }
        let _ = self.width_i32()?;
        let _ = self.height_i32()?;
        Ok(())
    }

    fn width_i32(self) -> Result<i32> {
        self.width
            .try_into()
            .context("screenshot area width exceeds GNOME Shell integer range")
    }

    fn height_i32(self) -> Result<i32> {
        self.height
            .try_into()
            .context("screenshot area height exceeds GNOME Shell integer range")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("codex-screenshot-test-{name}-{}", unique_suffix()))
    }

    fn valid_png(width: u32, height: u32) -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        png.extend_from_slice(&13_u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&width.to_be_bytes());
        png.extend_from_slice(&height.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        png
    }

    fn valid_rgba_png(width: u32, height: u32) -> Vec<u8> {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![128; (width * height * 4) as usize])
                .unwrap();
        }
        png
    }

    #[test]
    fn decodes_file_uri_percent_escapes() {
        assert_eq!(
            file_uri_to_path("file:///tmp/Codex%20Screenshot.png").unwrap(),
            PathBuf::from("/tmp/Codex Screenshot.png")
        );
    }

    #[test]
    fn request_token_is_portal_safe() {
        let token = request_token();
        assert!(token.starts_with("computer_use_linux_"));
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    #[test]
    fn reads_png_dimensions_from_ihdr() {
        let png = valid_png(3840, 1080);

        assert_eq!(png_dimensions(&png).unwrap(), (3840, 1080));
    }

    #[tokio::test]
    async fn portal_capture_preserves_valid_returned_path() {
        let path = test_path("portal-valid");
        fs::write(&path, valid_png(1, 1)).unwrap();

        let capture = read_png_as_capture(
            path.clone(),
            "xdg-desktop-portal",
            ScreenshotCleanup::Preserve,
            None,
        )
        .await
        .unwrap();

        assert_eq!(capture.source, "xdg-desktop-portal");
        assert!(path.exists());
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn portal_capture_preserves_invalid_returned_path() {
        let path = test_path("portal-invalid");
        fs::write(&path, b"").unwrap();

        let error = read_png_as_capture(
            path.clone(),
            "xdg-desktop-portal",
            ScreenshotCleanup::Preserve,
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("screenshot file was empty"));
        assert!(path.exists());
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn gnome_capture_deletes_backend_temp_path_on_success() {
        let path = test_path("gnome-valid");
        fs::write(&path, valid_png(1, 1)).unwrap();

        let capture = read_png_as_capture(
            path.clone(),
            "gnome-shell",
            ScreenshotCleanup::DeletePath(path.clone()),
            None,
        )
        .await
        .unwrap();

        assert_eq!(capture.source, "gnome-shell");
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn gnome_capture_deletes_backend_temp_path_on_parse_failure() {
        let path = test_path("gnome-invalid");
        fs::write(&path, b"").unwrap();

        let error = read_png_as_capture(
            path.clone(),
            "gnome-shell",
            ScreenshotCleanup::DeletePath(path.clone()),
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("screenshot file was empty"));
        assert!(!path.exists());
    }

    #[test]
    fn gnome_failure_cleanup_removes_requested_temp_path() {
        let path = test_path("gnome-pre-read-failure");
        fs::write(&path, b"partial").unwrap();

        cleanup_gnome_requested_path(&path);

        assert!(!path.exists());
    }

    #[tokio::test]
    async fn gnome_deletes_requested_temp_path_and_preserves_unexpected_returned_path() {
        let requested = test_path("gnome-requested");
        let returned = test_path("gnome-returned");
        fs::write(&requested, b"partial").unwrap();
        fs::write(&returned, valid_png(1, 1)).unwrap();

        let capture = read_png_as_capture(
            returned.clone(),
            "gnome-shell",
            ScreenshotCleanup::DeletePath(requested.clone()),
            None,
        )
        .await
        .unwrap();

        assert_eq!(capture.source, "gnome-shell");
        assert!(!requested.exists());
        assert!(returned.exists());
        let _ = fs::remove_file(returned);
    }

    #[test]
    fn validates_screenshot_area_dimensions() {
        let error = ScreenshotArea {
            x: 0,
            y: 0,
            width: 0,
            height: 10,
        }
        .validate()
        .unwrap_err();

        assert!(error.to_string().contains("positive width and height"));
    }

    #[tokio::test]
    async fn capture_records_requested_area_metadata() {
        let path = test_path("area-metadata");
        fs::write(&path, valid_png(20, 10)).unwrap();
        let area = ScreenshotArea {
            x: 7,
            y: 9,
            width: 20,
            height: 10,
        };

        let capture = read_png_as_capture(
            path.clone(),
            "gnome-shell-area",
            ScreenshotCleanup::Preserve,
            Some(area),
        )
        .await
        .unwrap();

        assert_eq!(capture.source, "gnome-shell-area");
        assert_eq!(capture.region, Some(area));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn crop_png_bytes_clips_requested_area_to_image_bounds() {
        let png = valid_rgba_png(4, 3);

        let (cropped, area) = crop_png_bytes(
            &png,
            ScreenshotArea {
                x: 2,
                y: 1,
                width: 5,
                height: 5,
            },
        )
        .unwrap();

        assert_eq!(
            area,
            ScreenshotArea {
                x: 2,
                y: 1,
                width: 2,
                height: 2,
            }
        );
        assert_eq!(png_dimensions(&cropped).unwrap(), (2, 2));
    }

    #[test]
    fn crop_capture_marks_source_and_region() {
        let png = valid_rgba_png(10, 10);
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(png));
        let capture = ScreenshotCapture {
            mime_type: "image/png".to_string(),
            data_url,
            source: "xdg-desktop-portal".to_string(),
            width: 10,
            height: 10,
            region: None,
        };

        let cropped = crop_capture(
            capture,
            ScreenshotArea {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
        )
        .unwrap();

        assert_eq!(cropped.source, "xdg-desktop-portal-crop");
        assert_eq!(cropped.width, 3);
        assert_eq!(cropped.height, 4);
        assert_eq!(
            cropped.region,
            Some(ScreenshotArea {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            })
        );
    }
}
