#[cfg(all(unix, not(target_os = "macos")))]
use std::path::Path;
use std::{
    process::{Command, Stdio},
    thread,
};

use anyhow::{Context, Result};

/// Open a preview URL in the configured browser.
///
/// The generic `open` crate delegates explicit app launches to platform helpers. That is fine for
/// default browser opening, but explicit browser commands need a little more control so Penview can
/// prefer a new tab in an existing browser process instead of accidentally launching a fresh window.
pub fn open_url(url: &str, browser: Option<&str>) -> Result<()> {
    match browser.map(str::trim).filter(|browser| !browser.is_empty()) {
        Some(browser) => spawn_launch_command(browser_launch_command(url, browser)),
        None => open::that_detached(url)
            .with_context(|| format!("failed to open {url} with the default browser")),
    }
}

fn spawn_launch_command(launch: LaunchCommand) -> Result<()> {
    let mut command = Command::new(&launch.program);
    command
        .args(&launch.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to launch browser command `{}`", launch.display()))?;

    // Reap short-lived browser wrapper processes without blocking the preview server. If the
    // spawned process is the long-running browser itself, this background waiter simply exits when
    // the browser does.
    thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(())
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LaunchCommand {
    program: String,
    args: Vec<String>,
}

impl LaunchCommand {
    fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(target_os = "macos")]
fn browser_launch_command(url: &str, browser: &str) -> LaunchCommand {
    // Use the canonical argument order. `open::with` currently builds `open <url> -a <app>`;
    // `open -a <app> <url>` is the form that reliably targets an existing application instance.
    LaunchCommand {
        program: "/usr/bin/open".to_string(),
        args: vec!["-a".to_string(), browser.to_string(), url.to_string()],
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn browser_launch_command(url: &str, browser: &str) -> LaunchCommand {
    let (program, mut args) = split_browser_command(browser);

    if supports_new_tab_flag(&program, &args) && !has_explicit_window_or_tab_arg(&args) {
        args.push("--new-tab".to_string());
    }
    args.push(url.to_string());

    LaunchCommand { program, args }
}

#[cfg(windows)]
fn browser_launch_command(url: &str, browser: &str) -> LaunchCommand {
    LaunchCommand {
        program: browser.to_string(),
        args: vec![url.to_string()],
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn split_browser_command(browser: &str) -> (String, Vec<String>) {
    let mut parts = browser.split_whitespace();
    let program = parts.next().unwrap_or(browser).to_string();
    let args = parts.map(ToString::to_string).collect();
    (program, args)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn supports_new_tab_flag(program: &str, args: &[String]) -> bool {
    let command_text = std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    let program_name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase();

    [
        "firefox",
        "librewolf",
        "waterfox",
        "iceweasel",
        "zen-browser",
        "chromium",
        "google-chrome",
        "chrome",
        "brave-browser",
        "brave",
        "vivaldi",
        "opera",
        "microsoft-edge",
        "msedge",
    ]
    .iter()
    .any(|browser| program_name.contains(browser) || command_text.contains(browser))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn has_explicit_window_or_tab_arg(args: &[String]) -> bool {
    args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--new-tab" | "--new-window" | "--app" | "--kiosk"
        ) || arg.starts_with("--app=")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "http://127.0.0.1:9876/?path=%2Ftmp%2Fnote.md";

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_uses_open_app_before_url() {
        assert_eq!(
            browser_launch_command(URL, "Firefox"),
            LaunchCommand {
                program: "/usr/bin/open".to_string(),
                args: vec!["-a".to_string(), "Firefox".to_string(), URL.to_string()],
            }
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn firefox_prefers_new_tab() {
        assert_eq!(
            browser_launch_command(URL, "firefox"),
            LaunchCommand {
                program: "firefox".to_string(),
                args: vec!["--new-tab".to_string(), URL.to_string()],
            }
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn browser_args_are_preserved_before_new_tab() {
        assert_eq!(
            browser_launch_command(URL, "firefox -P work"),
            LaunchCommand {
                program: "firefox".to_string(),
                args: vec![
                    "-P".to_string(),
                    "work".to_string(),
                    "--new-tab".to_string(),
                    URL.to_string()
                ],
            }
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn explicit_window_or_tab_arg_is_not_overridden() {
        assert_eq!(
            browser_launch_command(URL, "firefox --new-window"),
            LaunchCommand {
                program: "firefox".to_string(),
                args: vec!["--new-window".to_string(), URL.to_string()],
            }
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn unknown_browser_gets_plain_url_arg() {
        assert_eq!(
            browser_launch_command(URL, "xdg-open"),
            LaunchCommand {
                program: "xdg-open".to_string(),
                args: vec![URL.to_string()],
            }
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn flatpak_browser_app_id_is_detected() {
        assert_eq!(
            browser_launch_command(URL, "flatpak run org.mozilla.firefox"),
            LaunchCommand {
                program: "flatpak".to_string(),
                args: vec![
                    "run".to_string(),
                    "org.mozilla.firefox".to_string(),
                    "--new-tab".to_string(),
                    URL.to_string(),
                ],
            }
        );
    }
}
