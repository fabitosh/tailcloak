mod config;
mod tailscale;
mod wifi;

fn main() {
    let config = config::Config::load().expect("failed to load config");
    println!("trusted SSIDs: {:?}", config.trusted_ssids);
    let current_ssid = wifi::get_current_ssid();
    let is_trusted: bool = match &current_ssid {
        Some(current_ssid) => config.trusted_ssids.contains(current_ssid),
        None => false, // no wifi = not trusted
    };

    if is_trusted {
        let _ = tailscale::down();
    } else {
        let _ = tailscale::up();
    }
}
