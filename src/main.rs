mod atspi_tree;
mod diagnostics;
mod gnome_extension;
mod remote_desktop;
mod screenshot;
mod server;
mod terminal;
mod windows;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::{Path, PathBuf};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    diagnostics::hydrate_session_bus_env();

    match std::env::args().nth(1).as_deref() {
        Some("mcp") => server::serve_mcp().await,
        Some("doctor") => {
            let report = diagnostics::doctor_report();
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .context("failed to serialize doctor report")?
            );
            Ok(())
        }
        Some("setup") => {
            let report = diagnostics::setup_accessibility_report();
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .context("failed to serialize setup report")?
            );
            Ok(())
        }
        Some("apps") => {
            let apps = atspi_tree::list_accessible_apps(50).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&apps)
                    .context("failed to serialize accessible apps")?
            );
            Ok(())
        }
        Some("state") => {
            let app_name_or_bundle_identifier = std::env::args().nth(2);
            let nodes =
                atspi_tree::snapshot_tree(app_name_or_bundle_identifier.as_deref(), None, 120, 12)
                    .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&nodes)
                    .context("failed to serialize accessibility tree")?
            );
            Ok(())
        }
        Some("screenshot") => {
            let (args, output_path) = split_output_arg(std::env::args().skip(2).collect())?;
            if !args.is_empty() {
                bail!("usage: computer-use-linux screenshot [--output FILE]");
            }
            let capture = screenshot::capture_screenshot().await?;
            write_capture_if_requested(&capture, output_path.as_deref())?;
            print_screenshot_report(capture)?;
            Ok(())
        }
        Some("screenshot-area") => {
            let (args, output_path) = split_output_arg(std::env::args().skip(2).collect())?;
            if args.len() != 4 {
                bail!(
                    "usage: computer-use-linux screenshot-area X Y WIDTH HEIGHT [--output FILE]"
                );
            }
            let area = screenshot::ScreenshotArea {
                x: parse_arg(&args[0], "X")?,
                y: parse_arg(&args[1], "Y")?,
                width: parse_arg(&args[2], "WIDTH")?,
                height: parse_arg(&args[3], "HEIGHT")?,
            };
            let capture = screenshot::capture_screenshot_area(area).await?;
            write_capture_if_requested(&capture, output_path.as_deref())?;
            print_screenshot_report(capture)?;
            Ok(())
        }
        Some("screenshot-window") => {
            let (args, output_path) = split_output_arg(std::env::args().skip(2).collect())?;
            if args.len() != 1 {
                bail!(
                    "usage: computer-use-linux screenshot-window WINDOW_ID [--output FILE]"
                );
            }
            let window_id = args[0]
                .parse::<u64>()
                .context("WINDOW_ID must be a positive integer")?;
            let windows = windows::list_windows().await?;
            let window = windows::resolve_window_target(
                &windows,
                &windows::WindowTarget {
                    window_id: Some(window_id),
                    ..Default::default()
                },
            )?;
            let bounds = window
                .bounds
                .as_ref()
                .with_context(|| format!("window {window_id} has no known bounds"))?;
            let x = bounds
                .x
                .with_context(|| format!("window {window_id} has no known x coordinate"))?;
            let y = bounds
                .y
                .with_context(|| format!("window {window_id} has no known y coordinate"))?;
            let capture = screenshot::capture_screenshot_area(screenshot::ScreenshotArea {
                x,
                y,
                width: bounds.width,
                height: bounds.height,
            })
            .await?;
            write_capture_if_requested(&capture, output_path.as_deref())?;
            print_screenshot_report(capture)?;
            Ok(())
        }
        Some("windows") => {
            let report = match windows::list_windows().await {
                Ok(windows) => {
                    let backend = windows
                        .first()
                        .map(|window| window.backend.as_str())
                        .unwrap_or(windows::GNOME_SHELL_INTROSPECT_BACKEND);
                    serde_json::json!({
                        "backend": backend,
                        "windows": windows,
                        "error": null,
                        "permissions_hint": null,
                    })
                }
                Err(error) => {
                    let error = format!("{error:#}");
                    serde_json::json!({
                        "backend": windows::GNOME_SHELL_INTROSPECT_BACKEND,
                        "windows": [],
                        "error": error,
                        "permissions_hint": windows::window_permission_hint(&error),
                    })
                }
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        }
        Some("setup-window-targeting") => {
            let report = gnome_extension::setup_window_targeting_report().await;
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .context("failed to serialize window targeting setup report")?
            );
            Ok(())
        }
        Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some(command) => {
            anyhow::bail!(
                "unknown command '{command}'. Expected one of: mcp, doctor, setup, apps, state, screenshot, screenshot-area, screenshot-window, windows, setup-window-targeting"
            );
        }
        None => {
            print_help();
            Ok(())
        }
    }
}

fn parse_arg<T>(value: &str, name: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value
        .parse::<T>()
        .with_context(|| format!("{name} must be a number"))
}

fn split_output_arg(args: Vec<String>) -> Result<(Vec<String>, Option<PathBuf>)> {
    let mut positional = Vec::new();
    let mut output_path = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        if arg == "--output" || arg == "-o" {
            if output_path.is_some() {
                bail!("pass --output only once");
            }
            let path = args.next().context("--output requires a file path")?;
            output_path = Some(PathBuf::from(path));
        } else {
            positional.push(arg);
        }
    }

    Ok((positional, output_path))
}

fn write_capture_if_requested(
    capture: &screenshot::ScreenshotCapture,
    output_path: Option<&Path>,
) -> Result<()> {
    let Some(output_path) = output_path else {
        return Ok(());
    };
    let payload = capture
        .data_url
        .strip_prefix("data:image/png;base64,")
        .context("screenshot data URL was not an image/png base64 payload")?;
    let bytes = STANDARD
        .decode(payload)
        .context("failed to decode screenshot data URL")?;
    std::fs::write(output_path, bytes)
        .with_context(|| format!("failed to write screenshot to {}", output_path.display()))?;
    Ok(())
}

fn print_screenshot_report(capture: screenshot::ScreenshotCapture) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "mime_type": capture.mime_type,
            "source": capture.source,
            "width": capture.width,
            "height": capture.height,
            "region": capture.region,
            "data_url_length": capture.data_url.len()
        }))
        .context("failed to serialize screenshot report")?
    );
    Ok(())
}

fn print_help() {
    println!(
        "computer-use-linux\n\nUsage:\n  computer-use-linux mcp\n  computer-use-linux doctor\n  computer-use-linux setup\n  computer-use-linux setup-window-targeting\n  computer-use-linux apps\n  computer-use-linux state [APP_NAME]\n  computer-use-linux screenshot [--output FILE]\n  computer-use-linux screenshot-area X Y WIDTH HEIGHT [--output FILE]\n  computer-use-linux screenshot-window WINDOW_ID [--output FILE]\n  computer-use-linux windows"
    );
}
