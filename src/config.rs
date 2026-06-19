use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::network::MacAddr;

#[derive(Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
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

    pub fn remove_trusted_gateway(&mut self, mac: MacAddr) -> bool {
        self.trusted_gateway_macs.remove(&mac)
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
    Ok(config_dir()?.join("config.toml"))
}

pub fn config_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(xdg_dir("XDG_CONFIG_HOME", ".config")?.join("tailcloak"))
}

/// runtime state (the pause window)
pub fn state_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(xdg_dir("XDG_STATE_HOME", ".local/state")?.join("tailcloak"))
}

fn xdg_dir(var: &str, fallback: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(dir) = env::var_os(var).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = dirs::home_dir().ok_or("could not determine home directory")?;
    Ok(home.join(fallback))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn mac(s: &str) -> MacAddr {
        s.parse().expect("valid test MAC")
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.add_trusted_gateway(mac("d8:ec:e5:af:d0:29"));
        config.add_trusted_gateway(mac("00:11:22:33:44:55"));
        config.save_to(&path).unwrap();

        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn load_from_missing_file_is_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.toml");

        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded, Config::default());
        assert!(loaded.trusted_gateway_macs.is_empty());
    }

    #[test]
    fn save_overwrites_prior_contents() {
        // The atomic rename must replace the file wholesale, not merge into it.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut first = Config::default();
        first.add_trusted_gateway(mac("d8:ec:e5:af:d0:29"));
        first.save_to(&path).unwrap();

        let mut second = Config::default();
        second.add_trusted_gateway(mac("00:11:22:33:44:55"));
        second.save_to(&path).unwrap();

        assert_eq!(Config::load_from(&path).unwrap(), second);
    }

    #[test]
    fn add_trusted_gateway_reports_novelty() {
        let mut config = Config::default();
        assert!(config.add_trusted_gateway(mac("d8:ec:e5:af:d0:29"))); // newly added
        assert!(!config.add_trusted_gateway(mac("d8:ec:e5:af:d0:29"))); // already present
        assert_eq!(config.trusted_gateway_macs.len(), 1);
    }

    #[test]
    fn remove_trusted_gateway_reports_presence() {
        let mut config = Config::default();
        config.add_trusted_gateway(mac("d8:ec:e5:af:d0:29"));
        assert!(config.remove_trusted_gateway(mac("d8:ec:e5:af:d0:29"))); // was present
        assert!(!config.remove_trusted_gateway(mac("d8:ec:e5:af:d0:29"))); // already gone
        assert!(config.trusted_gateway_macs.is_empty());
    }

    #[test]
    fn show_trusted_formats_a_single_mac() {
        // Multi-element order is unspecified (HashSet), so assert on one entry.
        let mut config = Config::default();
        config.add_trusted_gateway(mac("d8:ec:e5:af:d0:29"));
        assert_eq!(config.show_trusted(), "d8:ec:e5:af:d0:29");
    }
}
