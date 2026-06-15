use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct MacAddr(String);

impl FromStr for MacAddr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        let valid = parts.len() == 6
            && parts
                .iter()
                .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()));
        if !valid {
            if parts[0].contains("-") {
                return Err(format!(
                    "Unsupported or invald MAC address: {s}. Dash notation is not supported."
                ));
            }
            return Err(format!("invalid MAC address: {s}"));
        }
        Ok(MacAddr(s.to_ascii_lowercase()))
    }
}

impl From<netdev::MacAddr> for MacAddr {
    fn from(m: netdev::MacAddr) -> Self {
        MacAddr(m.address())
    }
}

pub fn current_mac_gateway() -> Option<MacAddr> {
    netdev::get_default_gateway()
        .ok()
        .map(|g| g.mac_addr.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_input_parses() {
        for valid in ["00:1a:2b:3c:4d:5e", "AA:BB:CC:DD:EE:FF"] {
            assert!(valid.parse::<MacAddr>().is_ok(), "should parse: {valid}");
        }
    }
    #[test]
    fn dash_format_unsupported() {
        let input = String::from("00-1A-2B-3C-4D-5E");
        assert!(input.parse::<MacAddr>().is_err());
    }
    #[test]
    fn rejects_malformed() {
        for bad in ["00:1a:2b:3c:4d", "AA:BB:CC:DD:EE:ZZ"] {
            let result = bad.parse::<MacAddr>();
            assert!(result.is_err(), "should not parse: {bad}");
        }
    }
}
