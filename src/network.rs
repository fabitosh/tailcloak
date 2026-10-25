use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use system_configuration::dynamic_store::SCDynamicStoreBuilder;

pub use netdev::MacAddr;

/// MAC of the physical network's default gateway, or `None` when no physical
/// gateway can be resolved (no link, or the router is not ARP-verified yet).
///
/// Read from SystemConfiguration rather than the ARP table: since macOS 27,
/// unentitled processes get an empty ARP dump and redacted link addresses.
/// `configd` still publishes each service's ARP-verified router MAC.
/// Tunnels (VPNs, Tailscale) never carry one, so they drop out naturally.
pub fn current_mac_gateway() -> Option<MacAddr> {
    let store = SCDynamicStoreBuilder::new(crate::launchd::LABEL).build()?;
    let key = CFString::from_static_string("ARPResolvedHardwareAddress");
    store
        .get_keys("State:/Network/Service/.*/IPv4")?
        .iter()
        .filter_map(|service| store.get(service.clone()))
        .filter_map(|plist| plist.downcast_into::<CFDictionary>())
        .filter_map(|dict| {
            let value = *dict.find(key.as_CFTypeRef())?;
            // SAFETY: `value` is a live CF object owned by `dict`; get-rule retains it.
            unsafe { CFType::wrap_under_get_rule(value) }.downcast::<CFString>()
        })
        .find_map(|mac| parse_resolved(&mac.to_string()))
}

/// Parses a gateway MAC usable as a network identity: a real unicast address,
/// not the all-zero placeholder.
fn parse_resolved(s: &str) -> Option<MacAddr> {
    let mac: MacAddr = s.parse().ok()?;
    (mac != MacAddr::zero() && mac.is_unicast()).then_some(mac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unicast_mac() {
        assert_eq!(
            parse_resolved("f8:0d:a9:c8:04:04"),
            Some("f8:0d:a9:c8:04:04".parse().unwrap())
        );
    }

    #[test]
    fn rejects_zero_mac() {
        assert_eq!(parse_resolved("00:00:00:00:00:00"), None);
    }

    #[test]
    fn rejects_multicast_mac() {
        assert_eq!(parse_resolved("01:00:5e:00:00:fb"), None);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_resolved("not-a-mac"), None);
    }
}
