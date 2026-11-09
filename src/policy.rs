//! What tailcloak enforces on a class of network (trusted / untrusted).
//!
//! A `Policy` is two orthogonal knobs: the Tailscale connection state and the
//! exit node. Each knob can be enforced on every network change or left
//! `unmanaged`. Contradictory combinations are rejected at parse time, so a
//! `Policy` value is always valid.

use serde::{Deserialize, Serialize};

/// Whether Tailscale is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Connection {
    Up,
    Down,
    /// Never call `tailscale up`/`down` — the user's manual choice stands.
    Unmanaged,
}

/// Which exit node Tailscale routes internet traffic through.
///
/// In TOML this is a plain string: `"off"` and `"unmanaged"` are reserved,
/// anything else names a node (IP, MagicDNS name, or `auto:any`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum ExitNode {
    /// Actively clear the exit node.
    Off,
    /// Leave whatever exit node is configured.
    Unmanaged,
    /// Actively set this exit node.
    Node(String),
}

impl TryFrom<String> for ExitNode {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "off" => Ok(ExitNode::Off),
            "unmanaged" => Ok(ExitNode::Unmanaged),
            "" => Err(r#"exit_node must not be empty; use "off" to clear it"#.into()),
            _ => Ok(ExitNode::Node(s)),
        }
    }
}

impl From<ExitNode> for String {
    fn from(exit_node: ExitNode) -> Self {
        match exit_node {
            ExitNode::Off => "off".into(),
            ExitNode::Unmanaged => "unmanaged".into(),
            ExitNode::Node(node) => node,
        }
    }
}

/// A validated policy. Fields are private so the only ways to build one are
/// `Policy::new` and deserialization, both of which reject contradictions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawPolicy", into = "RawPolicy")]
pub struct Policy {
    tailscale: Connection,
    exit_node: ExitNode,
}

/// The unvalidated TOML shape of a `[trusted]` / `[untrusted]` section.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicy {
    tailscale: Connection,
    exit_node: ExitNode,
}

impl Policy {
    pub fn new(tailscale: Connection, exit_node: ExitNode) -> Result<Self, String> {
        if tailscale == Connection::Down && matches!(exit_node, ExitNode::Node(_)) {
            return Err(
                r#"tailscale = "down" can't be combined with an exit node; use exit_node = "off" or "unmanaged""#
                    .into(),
            );
        }
        Ok(Policy {
            tailscale,
            exit_node,
        })
    }

    /// 0.1 behavior on trusted networks: take Tailscale down.
    pub fn trusted_default() -> Self {
        Policy {
            tailscale: Connection::Down,
            exit_node: ExitNode::Unmanaged,
        }
    }

    /// 0.1 behavior on untrusted networks: bring Tailscale up.
    pub fn untrusted_default() -> Self {
        Policy {
            tailscale: Connection::Up,
            exit_node: ExitNode::Unmanaged,
        }
    }

    pub fn tailscale(&self) -> Connection {
        self.tailscale
    }

    pub fn exit_node(&self) -> &ExitNode {
        &self.exit_node
    }
}

impl TryFrom<RawPolicy> for Policy {
    type Error = String;

    fn try_from(raw: RawPolicy) -> Result<Self, Self::Error> {
        Policy::new(raw.tailscale, raw.exit_node)
    }
}

impl From<Policy> for RawPolicy {
    fn from(policy: Policy) -> Self {
        RawPolicy {
            tailscale: policy.tailscale,
            exit_node: policy.exit_node,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml_src: &str) -> Result<Policy, toml::de::Error> {
        toml::from_str(toml_src)
    }

    #[test]
    fn exit_node_reserved_words_and_names() {
        assert_eq!(ExitNode::try_from("off".to_string()), Ok(ExitNode::Off));
        assert_eq!(
            ExitNode::try_from("unmanaged".to_string()),
            Ok(ExitNode::Unmanaged)
        );
        assert_eq!(
            ExitNode::try_from("auto:any".to_string()),
            Ok(ExitNode::Node("auto:any".into()))
        );
        assert!(ExitNode::try_from(String::new()).is_err());
    }

    #[test]
    fn exit_node_string_round_trips() {
        for s in ["off", "unmanaged", "my-node", "100.101.102.103"] {
            let node = ExitNode::try_from(s.to_string()).unwrap();
            assert_eq!(String::from(node), s);
        }
    }

    #[test]
    fn parses_a_valid_section() {
        let policy = parse("tailscale = \"up\"\nexit_node = \"auto:any\"").unwrap();
        assert_eq!(policy.tailscale(), Connection::Up);
        assert_eq!(policy.exit_node(), &ExitNode::Node("auto:any".into()));
    }

    #[test]
    fn rejects_down_with_an_exit_node() {
        let err = parse("tailscale = \"down\"\nexit_node = \"my-node\"").unwrap_err();
        assert!(err.to_string().contains("can't be combined"), "{err}");
    }

    #[test]
    fn allows_down_with_exit_node_off() {
        assert!(parse("tailscale = \"down\"\nexit_node = \"off\"").is_ok());
    }

    #[test]
    fn rejects_unknown_connection_state() {
        assert!(parse("tailscale = \"on\"\nexit_node = \"off\"").is_err());
    }

    #[test]
    fn rejects_unknown_keys() {
        // A typo must fail loudly, not silently drop the setting.
        assert!(parse("tailscale = \"up\"\nexit-node = \"my-node\"").is_err());
    }

    #[test]
    fn rejects_incomplete_section() {
        assert!(parse("exit_node = \"my-node\"").is_err());
    }
}
