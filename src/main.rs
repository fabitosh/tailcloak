mod config;
mod tailscale;
mod wifi;

fn main() {
    let config = config::Config::load().expect("failed to load config");
    println!("trusted SSIDs: {:?}", config.trusted_ssids);
    let current_ssid = wifi::get_current_ssid();
    let is_trusted = current_ssid
        .as_deref()
        .is_some_and(|s| config.trusted_ssids.contains(s)); // no wifi = not trusted

    if is_trusted {
        let _ = tailscale::down();
    } else {
        let _ = tailscale::up();
    }
}
