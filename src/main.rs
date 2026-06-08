mod config;
mod network;
mod tailscale;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load().expect("failed to load config");
    println!("trusted gateway MACs: {:?}", config.trusted_gateway_macs);
    let current_gateway = network::current_mac_gateway();
    let is_trusted = current_gateway.is_some_and(|m| config.trusted_gateway_macs.contains(&m)); // no gateway = not trusted

    if is_trusted {
        tailscale::down()?;
    } else {
        tailscale::up()?;
    }
    Ok(())
}
