use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::network::MacAddr;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub trusted_gateway_macs: HashSet<MacAddr>,
}

impl Config {
    pub fn load_or_default() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(&config_path()?)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = config_path()?;
        self.save_to(&path)
    }

    fn load_from(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
        match fs::read_to_string(path) {
            Ok(contents) => Ok(toml::from_str(&contents)?),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // ensure parent directories exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;

        // avoid half-written config file on crash
        let tmp = path.with_extension("toml.tmp");
        let write_and_swap = || -> Result<(), Box<dyn std::error::Error>> {
            fs::write(&tmp, &contents)?;
            fs::rename(&tmp, path)?;
            Ok(())
        };

        // remove tmp file if successful
        write_and_swap().inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
    }

    pub fn add_trusted_gateway(&mut self, mac: MacAddr) -> bool {
        self.trusted_gateway_macs.insert(mac)
    }

    pub fn show_trusted(&self) -> String {
        self.trusted_gateway_macs
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join(", ")
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
