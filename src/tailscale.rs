use std::process::Command;

pub fn up() -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("tailscale").arg("up").status()?;
    if !status.success() {
        return Err(format!("tailscale up exited with {status}").into());
    }
    Ok(())
}

pub fn down() -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("tailscale").arg("down").status()?;
    if !status.success() {
        return Err(format!("tailscale down exited with {status}").into());
    }
    Ok(())
}
