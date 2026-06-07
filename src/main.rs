mod config;
mod network;
mod tailscale;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load().expect("failed to load config");
    println!("trusted SSIDs: {:?}", config.trusted_ssids);
    let current_ssid = network::current_getaway_mac();
    let is_trusted = current_ssid.is_some_and(|s| config.trusted_ssids.contains(&s)); // no wifi = not trusted

    if is_trusted {
        tailscale::down()?;
    } else {
        tailscale::up()?;
    }
    Ok(())
}
