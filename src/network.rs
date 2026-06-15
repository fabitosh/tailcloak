use netdev::Interface;
use netdev::interface::types::InterfaceType;

pub use netdev::MacAddr;

/// MAC of the physical network's default gateway, or `None` when no physical
/// gateway can be resolved (no link, or its MAC is not yet in the ARP cache).
pub fn current_mac_gateway() -> Option<MacAddr> {
    physical_gateway_mac(&netdev::get_interfaces())
}

/// Picks the gateway MAC of a physical interface, ignoring VPN tunnels.
fn physical_gateway_mac(interfaces: &[Interface]) -> Option<MacAddr> {
    interfaces
        .iter()
        .filter(|iface| is_physical(&iface.if_type))
        .filter_map(|iface| iface.gateway.as_ref())
        .map(|gateway| gateway.mac_addr)
        .find(is_resolved)
}

/// A gateway MAC usable as a network identity: a real, resolved unicast
/// address. netdev yields the all-zero MAC when it cannot resolve the gateway
/// (tunnels, or a cold ARP cache right after joining a network), so reject it.
fn is_resolved(mac: &MacAddr) -> bool {
    *mac != MacAddr::zero() && mac.is_unicast()
}

/// Whether an interface is a physical link that can identify a network.
///
/// Excludes virtual interfaces — most importantly VPN tunnels, but also
/// loopback and peer-to-peer wireless (AWDL on macOS) — which never represent
/// the physical network we want to fingerprint. Everything else (the Ethernet
/// family, Wi-Fi, and even `Unknown`) is allowed and gated by [`is_resolved`].
fn is_physical(if_type: &InterfaceType) -> bool {
    !matches!(
        if_type,
        InterfaceType::Tunnel | InterfaceType::Loopback | InterfaceType::PeerToPeerWireless
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use netdev::NetworkDevice;

    /// Builds an interface carrying only the fields the selection logic reads.
    fn iface(if_type: InterfaceType, gateway_mac: Option<MacAddr>) -> Interface {
        let mut iface = Interface::dummy();
        iface.if_type = if_type;
        iface.gateway = gateway_mac.map(|mac| NetworkDevice {
            mac_addr: mac,
            ipv4: Vec::new(),
            ipv6: Vec::new(),
        });
        iface
    }

    fn mac(s: &str) -> MacAddr {
        s.parse().expect("valid test MAC")
    }

    #[test]
    fn picks_resolved_gateway_on_physical_interface() {
        let ifaces = [iface(
            InterfaceType::Wireless80211,
            Some(mac("d8:ec:e5:af:d0:29")),
        )];
        assert_eq!(
            physical_gateway_mac(&ifaces),
            Some(mac("d8:ec:e5:af:d0:29"))
        );
    }

    #[test]
    fn ignores_tunnel_and_picks_physical() {
        // Mirrors the real machine with Tailscale up: a zero-MAC tunnel default
        // route alongside the physical Wi-Fi gateway.
        let ifaces = [
            iface(InterfaceType::Tunnel, Some(MacAddr::zero())),
            iface(InterfaceType::Wireless80211, Some(mac("d8:ec:e5:af:d0:29"))),
        ];
        assert_eq!(
            physical_gateway_mac(&ifaces),
            Some(mac("d8:ec:e5:af:d0:29"))
        );
    }

    #[test]
    fn rejects_tunnel_even_with_nonzero_mac() {
        // The type filter must hold on its own, independent of the zero-MAC one.
        let ifaces = [iface(InterfaceType::Tunnel, Some(mac("d8:ec:e5:af:d0:29")))];
        assert_eq!(physical_gateway_mac(&ifaces), None);
    }

    #[test]
    fn rejects_unresolved_zero_mac() {
        // Cold ARP cache: physical interface, gateway present, MAC unresolved.
        let ifaces = [iface(InterfaceType::Ethernet, Some(MacAddr::zero()))];
        assert_eq!(physical_gateway_mac(&ifaces), None);
    }

    #[test]
    fn ignores_interface_without_gateway() {
        let ifaces = [iface(InterfaceType::Ethernet, None)];
        assert_eq!(physical_gateway_mac(&ifaces), None);
    }

    #[test]
    fn none_when_no_interfaces() {
        assert_eq!(physical_gateway_mac(&[]), None);
    }
}
