//! Temporary manual-override window.
//!
//! While a pause is active the daemon skips reconciliation, so a hand-run
//! `tailscale up`/`down` sticks instead of being re-asserted on the next network
//! change. State is a single Unix-timestamp file (`pause-until`) in the XDG
//! state dir — it's ephemeral runtime state, not config. Reads **fail open** — a
//! missing, expired, or unparseable file means "not paused" — so a glitch can
//! never suppress protection.

use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn pause_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(crate::config::state_dir()?.join("pause-until"))
}

/// Suspends reconciliation for `minutes`, starting now. `0` clears any pause.
pub fn set(minutes: u64) -> Result<(), Box<dyn std::error::Error>> {
    set_at(&pause_path()?, SystemTime::now(), minutes)
}

/// Removes the pause window, resuming reconciliation immediately.
pub fn clear() -> Result<(), Box<dyn std::error::Error>> {
    clear_at(&pause_path()?)
}

/// Time left in the current pause window, or `None` when not paused.
pub fn remaining() -> Option<Duration> {
    remaining_at(&pause_path().ok()?, SystemTime::now())
}

// --- pure seams (path + clock injected) so the logic is unit-testable ---

fn set_at(path: &Path, now: SystemTime, minutes: u64) -> Result<(), Box<dyn std::error::Error>> {
    if minutes == 0 {
        return clear_at(path);
    }
    let until = now + Duration::from_secs(minutes * 60);
    let secs = until.duration_since(UNIX_EPOCH)?.as_secs();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, secs.to_string())?;
    Ok(())
}

fn clear_at(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()), // already gone — fine
        Err(e) => Err(e.into()),
    }
}

fn remaining_at(path: &Path, now: SystemTime) -> Option<Duration> {
    let secs: u64 = fs::read_to_string(path).ok()?.trim().parse().ok()?;
    let until = UNIX_EPOCH + Duration::from_secs(secs);
    until.duration_since(now).ok() // Err == already elapsed -> None (not paused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn set_then_remaining_is_within_window() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pause-until");
        let now = SystemTime::now();
        set_at(&path, now, 5).unwrap();
        let left = remaining_at(&path, now).expect("should be paused");
        assert!(left.as_secs() > 0 && left.as_secs() <= 5 * 60);
    }

    #[test]
    fn expired_window_reads_as_not_paused() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pause-until");
        let start = SystemTime::now();
        set_at(&path, start, 5).unwrap();
        let later = start + Duration::from_secs(10 * 60); // window has elapsed
        assert!(remaining_at(&path, later).is_none());
    }

    #[test]
    fn zero_minutes_clears_any_pause() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pause-until");
        let now = SystemTime::now();
        set_at(&path, now, 5).unwrap();
        set_at(&path, now, 0).unwrap(); // resume
        assert!(remaining_at(&path, now).is_none());
        assert!(!path.exists());
    }

    #[test]
    fn missing_file_is_not_paused() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist");
        assert!(remaining_at(&path, SystemTime::now()).is_none());
    }

    #[test]
    fn garbage_file_fails_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pause-until");
        fs::write(&path, "not-a-number").unwrap();
        assert!(remaining_at(&path, SystemTime::now()).is_none());
    }
}
