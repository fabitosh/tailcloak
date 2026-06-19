#[cfg(not(target_os = "macos"))]
compile_error!("tailcloak only supports macOS (it relies on SystemConfiguration and the Tailscale CLI).");

mod config;
mod daemon;
mod launchd;
mod network;
mod pause;
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
        Some("service") => cmd_service(std::env::args().nth(2).as_deref()),
        Some("--help" | "-h" | "help") => {
            print_usage();
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("tailcloak {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(other) => {
            eprintln!("Unknown argument {other}\nRun `tailcloak --help` for usage.");
            std::process::exit(2);
        }
    }
}

fn print_usage() {
    println!(
        "tailcloak — toggle Tailscale based on trusted gateway MAC addresses (macOS)\n\
         \n\
         Usage: tailcloak [COMMAND]\n\
         \n\
         Commands:\n\
         \x20 (none)             Run the daemon in the foreground (what the LaunchAgent runs)\n\
         \x20 run-once           Reconcile once and exit\n\
         \x20 trust-current      Trust the current network's gateway\n\
         \x20 distrust-current   Untrust the current network's gateway\n\
         \x20 show-trusted       List trusted gateway MACs\n\
         \x20 pause <minutes>    Suspend toggling for N minutes (manual override); 0 resumes\n\
         \x20 resume             Resume toggling now\n\
         \x20 service install    Install and start the LaunchAgent\n\
         \x20 service uninstall  Stop and remove the LaunchAgent\n\
         \n\
         Options:\n\
         \x20 -h, --help         Print this help\n\
         \x20 -V, --version      Print version"
    );
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

/// `service <install|uninstall>` — namespaced so it reads as managing the
/// background agent, not the `tailcloak` binary itself (that's `cargo`'s job).
fn cmd_service(sub: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    match sub {
        Some("install") => launchd::install(),
        Some("uninstall") => launchd::uninstall(),
        other => {
            if let Some(other) = other {
                eprintln!("Unknown service command {other}");
            }
            eprintln!("Usage: tailcloak service <install|uninstall>");
            std::process::exit(2);
        }
    }
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
