use std::path::PathBuf;
use std::process::Command;

const CANDIDATE_INSTALLATION_PATHS: [&str; 4] = [
    "/usr/local/bin/tailscale",    // Tailscale.app CLI shim / Intel Homebrew
    "/opt/homebrew/bin/tailscale", // Apple Silicon Homebrew
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    "/usr/bin/tailscale",
];

pub fn resolve() -> Option<PathBuf> {
    CANDIDATE_INSTALLATION_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file()) // follows symlinks (Homebrew links into the Cellar)
}

fn run(arg: &str) -> Result<(), Box<dyn std::error::Error>> {
    let tailscale = resolve().ok_or("tailscale CLI not found")?;
    let status = Command::new(&tailscale).arg(arg).status()?;
    if !status.success() {
        return Err(format!("tailscale {arg} exited with {status}").into());
    }
    Ok(())
}

pub fn up() -> Result<(), Box<dyn std::error::Error>> {
    run("up")
}

pub fn down() -> Result<(), Box<dyn std::error::Error>> {
    run("down")
}
