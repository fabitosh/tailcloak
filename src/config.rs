use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
pub struct Config {
    pub trusted_ssids: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = config_path()?;
        let contents = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }
}
fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let base = xdg_config_home()?;
    Ok(base.join("tailcloak").join("config.toml"))
}

fn xdg_config_home() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = dirs::home_dir().ok_or("could not determine home directory")?;
    Ok(home.join(".config"))
}
