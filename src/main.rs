mod config;

fn main() {
    let config = config::Config::load().expect("failed to load config");
    println!("trusted SSIDs: {:?}", config.trusted_ssids);
}
