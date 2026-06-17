mod config;
mod network;
mod tailscale;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        None => run_daemon_once(),
        Some("trust-current") => cmd_trust_current(),
        Some("show-trusted") => cmd_show_trusted(),
        Some(other) => {
            eprintln!("Unknown argument {other}");
            std::process::exit(2);
        }
    }
}

fn cmd_trust_current() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = config::Config::load_or_default()?;
    let current_gateway = network::current_mac_gateway()
        .ok_or("No physical gateway found - are you on a network?")?;
    Ok(())
}

fn cmd_show_trusted() -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}

fn run_daemon_once() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load_or_default().expect("failed to load config");
    let trusted: Vec<String> = config
        .trusted_gateway_macs
        .iter()
        .map(|mac| mac.to_string())
        .collect();
    println!("trusted gateway MACs: [{}]", trusted.join(", "));

    let current_gateway = network::current_mac_gateway();
    match &current_gateway {
        Some(mac) => println!("current gateway: {mac}"),
        None => println!("current gateway: none (no physical gateway resolved)"),
    }

    let is_trusted = current_gateway.is_some_and(|m| config.trusted_gateway_macs.contains(&m)); // no gateway = not trusted

    if is_trusted {
        tailscale::down()?;
    } else {
        tailscale::up()?;
    }
    Ok(())
}
