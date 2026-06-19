//! launchd LaunchAgent install/uninstall (macOS).
//!
//! `install()` renders a LaunchAgent plist pointing at the *current* binary,
//! drops it in `~/Library/LaunchAgents/`, and loads it via `launchctl`. Because
//! the plist is generated from `env::current_exe()` at install time, there are
//! no placeholder paths to substitute and no symlink to maintain.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, io};

/// Reverse-DNS service identifier. Single source of truth for the launchd job
/// `Label`, the plist filename, and the `SCDynamicStore` session name.
/// Changing branding later is a one-line edit here (plus a reinstall).
pub const LABEL: &str = "dev.fmeier.tailcloak";

/// `~/Library/LaunchAgents/<LABEL>.plist` — an Apple path, resolved from `$HOME`
/// directly (not via XDG, unlike our config).
fn plist_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(home()?.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

/// `~/Library/Logs/tailcloak.log` — where launchd redirects stdout + stderr.
fn log_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(home()?.join("Library/Logs/tailcloak.log"))
}

fn home() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env::var_os("HOME").ok_or("HOME is not set")?))
}

/// Numeric UID, needed for the `gui/<uid>` launchd domain target. `getuid` is
/// not in std, so we shell out rather than pull in `libc` for one call.
fn current_uid() -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("id").arg("-u").output()?;
    if !out.status.success() {
        return Err("`id -u` failed".into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

/// Renders the LaunchAgent plist. Pure (no I/O) so it's cheap to unit-test.
///
/// Assumes `exe`/`log` contain no XML metacharacters (`& < >`); true for normal
/// macOS home paths. If that ever stops holding, escape before interpolating.
fn plist_contents(exe: &Path, log: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        exe = exe.display(),
        log = log.display(),
    )
}

/// Writes the plist for the current binary and (re)loads it as a LaunchAgent.
/// Idempotent: an already-loaded job is booted out first so `bootstrap` won't
/// fail with "service already loaded".
pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    if crate::tailscale::resolve().is_none() {
        eprintln!(
            "tailcloak: WARNING — `tailscale` CLI not found in any standard location.\n\
             The daemon cannot toggle Tailscale until it is installed \
             (https://tailscale.com/download).\n\
             Installing the agent anyway; it will start working once Tailscale is present."
        );
    }

    let exe = env::current_exe()?.canonicalize()?;
    let plist = plist_path()?;
    let log = log_path()?;

    if let Some(dir) = plist.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&plist, plist_contents(&exe, &log))?;

    let domain = format!("gui/{}", current_uid()?);
    let target = format!("{domain}/{LABEL}");

    // Best-effort unload of any prior version; output swallowed since "not
    // loaded" is the normal first-install case.
    let _ = Command::new("launchctl").args(["bootout", &target]).output();

    let plist_str = plist.to_str().ok_or("plist path is not valid UTF-8")?;
    let status = Command::new("launchctl")
        .args(["bootstrap", &domain, plist_str])
        .status()?;
    if !status.success() {
        return Err(format!("launchctl bootstrap failed ({status})").into());
    }

    println!("tailcloak: installed and started");
    println!("  agent: {}", plist.display());
    println!("  logs:  {}", log.display());
    Ok(())
}

/// Unloads the LaunchAgent and removes its plist. Safe to run when nothing is
/// installed.
pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let plist = plist_path()?;
    let target = format!("gui/{}/{LABEL}", current_uid()?);

    let _ = Command::new("launchctl").args(["bootout", &target]).output();

    match fs::remove_file(&plist) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {} // already gone — fine
        Err(e) => return Err(e.into()),
    }
    println!("tailcloak: uninstalled");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_embeds_binary_path_log_and_label() {
        let xml = plist_contents(
            Path::new("/Users/me/.cargo/bin/tailcloak"),
            Path::new("/Users/me/Library/Logs/tailcloak.log"),
        );
        assert!(xml.contains("<string>/Users/me/.cargo/bin/tailcloak</string>"));
        assert!(xml.contains("/Users/me/Library/Logs/tailcloak.log"));
        assert!(xml.contains(&format!("<string>{LABEL}</string>")));
        assert!(xml.contains("<key>RunAtLoad</key>\n    <true/>"));
    }
}
