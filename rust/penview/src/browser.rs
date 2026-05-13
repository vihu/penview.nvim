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
    browser_launch_command_with_path_exists(url, browser, |candidate| Path::new(candidate).exists())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn browser_launch_command_with_path_exists(
    url: &str,
    browser: &str,
    path_exists: impl Fn(&str) -> bool,
) -> LaunchCommand {
    let (program, mut args) = split_browser_command(browser, path_exists);

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
fn split_browser_command(
    browser: &str,
    path_exists: impl Fn(&str) -> bool,
) -> (String, Vec<String>) {
    let browser = browser.trim();

    if path_exists(browser) {
        return (browser.to_string(), Vec::new());
    }

    // Browser values often come from editor config rather than a shell. Preserve an unquoted
    // executable path containing spaces when it exists, e.g. WSL paths like:
    // /mnt/c/Program Files/Zen Browser/zen.exe
    if let Some((program, rest)) = split_existing_program_prefix(browser, &path_exists) {
        return (program.to_string(), split_command_words(rest));
    }

    let mut parts = split_command_words(browser).into_iter();
    let program = parts.next().unwrap_or_else(|| browser.to_string());
    let args = parts.collect();
    (program, args)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn split_existing_program_prefix(
    browser: &str,
    path_exists: impl Fn(&str) -> bool,
) -> Option<(&str, &str)> {
    let mut longest_match = None;

    for (idx, ch) in browser.char_indices() {
        if !ch.is_whitespace() {
            continue;
        }

        let candidate = browser[..idx].trim_end();
        if candidate.is_empty() || !path_exists(candidate) {
            continue;
        }

        longest_match = Some((candidate, browser[idx..].trim_start()));
    }

    longest_match
}

#[cfg(all(unix, not(target_os = "macos")))]
fn split_command_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match quote {
            Some(quote_char) if ch == quote_char => quote = None,
            Some(_) => current.push(ch),
            None if ch == '\\' => escaped = true,
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if escaped {
        current.push('\\');
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
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
        "zen.exe",
        "zen_browser",
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
    fn wsl_windows_browser_path_with_spaces_is_preserved() {
        let browser = "/mnt/c/Program Files/Zen Browser/zen.exe";

        assert_eq!(
            browser_launch_command_with_path_exists(URL, browser, |candidate| candidate == browser),
            LaunchCommand {
                program: browser.to_string(),
                args: vec!["--new-tab".to_string(), URL.to_string()],
            }
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn wsl_windows_browser_path_with_spaces_preserves_extra_args() {
        let browser = "/mnt/c/Program Files/Zen Browser/zen.exe";

        assert_eq!(
            browser_launch_command_with_path_exists(
                URL,
                &format!("{browser} --profile default"),
                |candidate| candidate == browser,
            ),
            LaunchCommand {
                program: browser.to_string(),
                args: vec![
                    "--profile".to_string(),
                    "default".to_string(),
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
