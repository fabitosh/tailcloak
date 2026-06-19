#[cfg(not(target_os = "macos"))]
compile_error!("tailcloak only supports macOS (it relies on SystemConfiguration and the Tailscale CLI).");

mod config;
mod daemon;
mod launchd;
mod network;
mod tailscale;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        None => daemon::run(),
        Some("run-once") => daemon::run_once(),
        Some("trust-current") => cmd_trust_current(),
        Some("distrust-current") => cmd_distrust_current(),
        Some("show-trusted") => cmd_show_trusted(),
        Some("pause") => cmd_pause(std::env::args().nth(2).as_deref()),
        Some("resume") => {
            pause::clear()?;
            println!("tailcloak: resumed");
            Ok(())
        }
        Some("install") => launchd::install(),
        Some("uninstall") => launchd::uninstall(),
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
    if config.add_trusted_gateway(current_gateway) {
        config.save()?;
        println!("Now trusting gateway {current_gateway}");
    } else {
        println!("Gateway {current_gateway} is already trusted")
    }
    Ok(())
}

fn cmd_distrust_current() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = config::Config::load_or_default()?;
    let current_gateway = network::current_mac_gateway()
        .ok_or("No physical gateway found - are you on a network?")?;
    if config.remove_trusted_gateway(current_gateway) {
        config.save()?;
        println!("No longer trusting gateway {current_gateway}");
    } else {
        println!("Gateway {current_gateway} was not trusted");
    }
    Ok(())
}

fn cmd_show_trusted() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load_or_default()?;
    let trusted = config.show_trusted();
    println!("trusted gateway MACs: {trusted}");
    Ok(())
}
fn cmd_pause(arg: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let minutes: u64 = arg
        .ok_or("usage: tailcloak pause <minutes>")?
        .parse()
        .map_err(|_| "pause expects a whole number of minutes")?;
    pause::set(minutes)?;
    match minutes {
        0 => println!("tailcloak: resumed"),
        _ => println!(
            "tailcloak: paused for {minutes} min — manual `tailscale up`/`down` will stick until it ends"
        ),
    }
    Ok(())
}
