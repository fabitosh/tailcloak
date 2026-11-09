//! Event-driven daemon acting on physical network changes.

use std::borrow::Cow;
use std::fmt;

use crate::config::Config;
use crate::network::{self, MacAddr};
use crate::pause;
use crate::policy::{Connection, ExitNode, Policy};
use crate::tailscale::Action;

use core_foundation::array::CFArray;
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_foundation::string::CFString;
use system_configuration::dynamic_store::{
    SCDynamicStore, SCDynamicStoreBuilder, SCDynamicStoreCallBackContext,
};

/// SystemConfiguration keys whose changes signal the network may have switched:
const WATCHED_KEYS: [&str; 2] = ["State:/Network/Global/IPv4", "State:/Network/Global/IPv6"];

/// Runs the event-driven daemon: reconciles the current network
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    reconcile(); // reconcile the current network before we start watching

    let store = SCDynamicStoreBuilder::new(crate::launchd::LABEL)
        .callback_context(SCDynamicStoreCallBackContext {
            callout: on_network_change,
            info: (),
        })
        .build()
        .ok_or("could not create SCDynamicStore session")?;

    let cf_keys = WATCHED_KEYS.map(CFString::from_static_string);
    let keys = CFArray::from_CFTypes(&cf_keys);
    let patterns = CFArray::<CFString>::from_CFTypes(&[]);
    if !store.set_notification_keys(&keys, &patterns) {
        return Err("could not register SystemConfiguration notification keys".into());
    }

    let source = store
        .create_run_loop_source()
        .ok_or("could not create run loop source")?;
    CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });

    println!("tailcloak: watching for network changes (Ctrl-C to stop)");
    CFRunLoop::run_current(); // blocks until the process is terminated
    Ok(())
}

/// Reconciles a single time, without watching. Useful for testing and as a
/// manual one-shot trigger.
pub fn run_once() -> Result<(), Box<dyn std::error::Error>> {
    reconcile();
    Ok(())
}

fn on_network_change(_store: SCDynamicStore, _changed_keys: CFArray<CFString>, _info: &mut ()) {
    reconcile();
}

/// Errors are logged, never propagated — one failure must not bring the daemon down.
fn reconcile() {
    if let Some(left) = pause::remaining() {
        println!(
            "tailcloak: paused ({} min left) — leaving Tailscale unchanged",
            left.as_secs().div_ceil(60)
        );
        return;
    }

    let gateway = network::current_mac_gateway();
    let config = Config::load_or_default()
        .inspect_err(|e| eprintln!("tailcloak: failed to load config: {e}"))
        .ok();

    let (verdict, policy) = select(config.as_ref(), gateway);
    let gw = gateway.map_or_else(|| "none".to_string(), |mac| mac.to_string());
    apply(&format!("gateway {gw} {verdict}"), &policy);
}

/// How the current network was classified, for the log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Trusted,
    Untrusted,
    /// Config couldn't be loaded — trust nothing.
    ConfigUnreadable,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Verdict::Trusted => "trusted",
            Verdict::Untrusted => "untrusted",
            Verdict::ConfigUnreadable => "untrusted (config unreadable, applying default)",
        })
    }
}

/// Pure: which policy applies. `config` is `None` when it couldn't be loaded;
/// then we fail safe to the untrusted default (Tailscale up) rather than doing
/// nothing. No gateway is never trusted.
fn select(config: Option<&Config>, gateway: Option<MacAddr>) -> (Verdict, Cow<'_, Policy>) {
    let Some(config) = config else {
        return (
            Verdict::ConfigUnreadable,
            Cow::Owned(Policy::untrusted_default()),
        );
    };
    let trusted = gateway.is_some_and(|mac| config.trusted_gateway_macs.contains(&mac));
    let verdict = if trusted {
        Verdict::Trusted
    } else {
        Verdict::Untrusted
    };
    (verdict, Cow::Borrowed(config.policy_for(trusted)))
}

/// Pure: the commands that enforce `policy`, in order. Connection comes first
/// so `up` precedes setting a named exit node (which needs the netmap);
/// `tailscale set` works while down, so clearing after `down` is fine too.
fn actions(policy: &Policy) -> Vec<Action<'_>> {
    let mut actions = Vec::new();
    match policy.tailscale() {
        Connection::Up => actions.push(Action::Up),
        Connection::Down => actions.push(Action::Down),
        Connection::Unmanaged => {}
    }
    match policy.exit_node() {
        ExitNode::Off => actions.push(Action::SetExitNode(None)),
        ExitNode::Node(node) => actions.push(Action::SetExitNode(Some(node))),
        ExitNode::Unmanaged => {}
    }
    actions
}

/// Runs every action even if an earlier one fails, and logs one summary line.
fn apply(label: &str, policy: &Policy) {
    let actions = actions(policy);
    if actions.is_empty() {
        println!("tailcloak: {label} -> nothing to enforce");
        return;
    }
    let done: Vec<String> = actions
        .iter()
        .filter_map(|action| match action.run() {
            Ok(()) => Some(action.to_string()),
            Err(e) => {
                eprintln!("tailcloak: {action} failed: {e}");
                None
            }
        })
        .collect();
    let summary = if done.is_empty() {
        "nothing applied".to_string()
    } else {
        done.join(", ")
    };
    println!("tailcloak: {label} -> {summary}");
}

/// End-to-end decision tests: config text + current gateway -> the exact
/// `tailscale` argv lists the daemon would run. Everything but the process
/// spawn itself is covered.
#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "aa:bb:cc:dd:ee:01";
    const CAFE: &str = "aa:bb:cc:dd:ee:02";

    /// Runs the full pure pipeline: parse -> select -> actions -> argv.
    /// Unparseable TOML stands in for an unreadable config file.
    fn commands(config_toml: &str, gateway: Option<&str>) -> (Verdict, Vec<Vec<String>>) {
        let config = toml::from_str::<Config>(config_toml).ok();
        let gateway = gateway.map(|g| g.parse::<MacAddr>().expect("valid test MAC"));
        let (verdict, policy) = select(config.as_ref(), gateway);
        let argv = actions(&policy).iter().map(Action::args).collect();
        (verdict, argv)
    }

    /// Expected commands, one argv slice per `tailscale` invocation.
    type Argv<'a> = &'a [&'a [&'a str]];

    fn argv(expected: Argv) -> Vec<Vec<String>> {
        expected
            .iter()
            .map(|cmd| cmd.iter().map(|a| a.to_string()).collect())
            .collect()
    }

    /// Every valid (tailscale, exit_node) combination and the commands it must
    /// produce, in order. `down` + a named node is rejected at parse time.
    const POLICIES: &[(&str, &str, &[&[&str]])] = &[
        ("up", "off", &[&["up"], &["set", "--exit-node="]]),
        ("up", "unmanaged", &[&["up"]]),
        ("up", "my-node", &[&["up"], &["set", "--exit-node=my-node"]]),
        ("down", "off", &[&["down"], &["set", "--exit-node="]]),
        ("down", "unmanaged", &[&["down"]]),
        ("unmanaged", "off", &[&["set", "--exit-node="]]),
        ("unmanaged", "unmanaged", &[]),
        ("unmanaged", "my-node", &[&["set", "--exit-node=my-node"]]),
    ];

    /// A policy no row in `POLICIES` produces, placed in the *other* section,
    /// so picking the wrong section can never accidentally match.
    const SENTINEL: &str = "tailscale = \"up\"\nexit_node = \"sentinel-node\"";

    #[test]
    fn every_trusted_policy_produces_its_commands() {
        for (tailscale, exit_node, expected) in POLICIES {
            let config = format!(
                "trusted_gateway_macs = [\"{HOME}\"]\n\
                 [trusted]\ntailscale = \"{tailscale}\"\nexit_node = \"{exit_node}\"\n\
                 [untrusted]\n{SENTINEL}\n"
            );
            let (verdict, got) = commands(&config, Some(HOME));
            assert_eq!(verdict, Verdict::Trusted, "{tailscale}/{exit_node}");
            assert_eq!(got, argv(expected), "trusted {tailscale}/{exit_node}");
        }
    }

    #[test]
    fn every_untrusted_policy_produces_its_commands() {
        for (tailscale, exit_node, expected) in POLICIES {
            let config = format!(
                "trusted_gateway_macs = [\"{HOME}\"]\n\
                 [trusted]\n{SENTINEL}\n\
                 [untrusted]\ntailscale = \"{tailscale}\"\nexit_node = \"{exit_node}\"\n"
            );
            // An unknown gateway and no gateway at all are both untrusted.
            for gateway in [Some(CAFE), None] {
                let (verdict, got) = commands(&config, gateway);
                assert_eq!(
                    verdict,
                    Verdict::Untrusted,
                    "{tailscale}/{exit_node} {gateway:?}"
                );
                assert_eq!(
                    got,
                    argv(expected),
                    "untrusted {tailscale}/{exit_node} gateway {gateway:?}"
                );
            }
        }
    }

    /// Whole-config scenarios, including backward compat and fail-safe paths.
    #[test]
    fn scenarios() {
        let legacy = format!("trusted_gateway_macs = [\"{HOME}\"]");
        let intended = format!(
            "trusted_gateway_macs = [\"{HOME}\"]\n\
             [trusted]\ntailscale = \"unmanaged\"\nexit_node = \"off\"\n\
             [untrusted]\ntailscale = \"up\"\nexit_node = \"auto:any\"\n"
        );
        // A typo'd key: rejected by deny_unknown_fields -> config unreadable.
        let typo = format!(
            "trusted_gateway_macs = [\"{HOME}\"]\n\
             [trusted]\ntailscale = \"down\"\nexit-node = \"off\"\n"
        );
        // Contradictory policy: also unreadable, also fails safe.
        let contradictory = format!(
            "trusted_gateway_macs = [\"{HOME}\"]\n\
             [trusted]\ntailscale = \"down\"\nexit_node = \"my-node\"\n"
        );

        use Verdict::*;
        let cases: &[(&str, &str, Option<&str>, Verdict, Argv)] = &[
            // 0.1 config: unchanged behavior.
            ("legacy home", &legacy, Some(HOME), Trusted, &[&["down"]]),
            ("legacy cafe", &legacy, Some(CAFE), Untrusted, &[&["up"]]),
            ("legacy offline", &legacy, None, Untrusted, &[&["up"]]),
            // Missing config file == empty config: nothing trusted, always up.
            ("empty config", "", Some(HOME), Untrusted, &[&["up"]]),
            // The 0.2 setup: hands off at home except clearing the exit node.
            (
                "intended home",
                &intended,
                Some(HOME),
                Trusted,
                &[&["set", "--exit-node="]],
            ),
            (
                "intended cafe",
                &intended,
                Some(CAFE),
                Untrusted,
                &[&["up"], &["set", "--exit-node=auto:any"]],
            ),
            // Broken configs must not leave us unprotected, even at home.
            (
                "typo at home",
                &typo,
                Some(HOME),
                ConfigUnreadable,
                &[&["up"]],
            ),
            (
                "contradiction at home",
                &contradictory,
                Some(HOME),
                ConfigUnreadable,
                &[&["up"]],
            ),
            (
                "garbage",
                "not = [valid",
                Some(HOME),
                ConfigUnreadable,
                &[&["up"]],
            ),
        ];

        for (name, config, gateway, verdict, expected) in cases {
            let (got_verdict, got) = commands(config, *gateway);
            assert_eq!(got_verdict, *verdict, "{name}");
            assert_eq!(got, argv(expected), "{name}");
        }
    }
}
