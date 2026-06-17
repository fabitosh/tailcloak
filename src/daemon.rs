//! Event-driven daemon: toggles Tailscale when the trusted-ness of the current
//! physical network changes.

use crate::config::Config;
use crate::{network, tailscale};

use core_foundation::array::CFArray;
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_foundation::string::CFString;
use system_configuration::dynamic_store::{
    SCDynamicStore, SCDynamicStoreBuilder, SCDynamicStoreCallBackContext,
};

/// SystemConfiguration keys whose changes signal the network may have switched:
const WATCHED_KEYS: [&str; 2] = ["State:/Network/Global/IPv4", "State:/Network/Global/IPv6"];

/// State threaded through every callback for the daemon's lifetime.
///
/// `applied_trust` records the trust decision behind our most recent toggle:
/// `None` before the first decision, then `Some(true)` (we ran `tailscale
/// down`) or `Some(false)` (we ran `tailscale up`). We only shell out to
/// Tailscale when this flips.
///
/// It is *not* required for correctness — `tailscale up`/`down` are idempotent,
/// so the daemon converges either way. But the watched keys fire on more than
/// network switches (toggling Tailscale itself re-fires this callback, as do
/// things like DHCP renewals), so the guard avoids a redundant subprocess spawn
/// and keeps the log to one line per genuine change.
struct State {
    applied_trust: Option<bool>,
}

/// Runs the event-driven daemon: applies policy to the current network, then
/// blocks on the run loop, re-applying whenever the watched state changes.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = State {
        applied_trust: None,
    };
    reconcile(&mut state); // apply to the current network before we start watching

    let store = SCDynamicStoreBuilder::new("com.fabitosh.tailcloak")
        .callback_context(SCDynamicStoreCallBackContext {
            callout: on_network_change,
            info: state,
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

/// Applies policy a single time, without watching. Useful for testing and as a
/// manual one-shot trigger.
pub fn run_once() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = State {
        applied_trust: None,
    };
    reconcile(&mut state);
    Ok(())
}

fn on_network_change(_store: SCDynamicStore, _changed_keys: CFArray<CFString>, state: &mut State) {
    reconcile(state);
}

/// Resolves the current physical gateway, decides trusted vs. not, and toggles
/// Tailscale — but only when the decision changed since last time. Errors are
/// logged, never propagated: one failure must not bring the daemon down.
fn reconcile(state: &mut State) {
    let config = match Config::load_or_default() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("tailcloak: failed to load config: {e}");
            return;
        }
    };

    let gateway = network::current_mac_gateway();
    let trusted = gateway.is_some_and(|mac| config.trusted_gateway_macs.contains(&mac));

    if state.applied_trust == Some(trusted) {
        // Same decision as last time: nothing to do. This is what filters the
        // self-triggered re-fire and other spurious notifications
        return;
    }

    let result = if trusted {
        tailscale::down()
    } else {
        tailscale::up()
    };
    match result {
        Ok(()) => {
            let gw = gateway.map_or_else(|| "none".to_string(), |mac| mac.to_string());
            let verdict = if trusted {
                "trusted -> tailscale down"
            } else {
                "untrusted -> tailscale up"
            };
            println!("tailcloak: gateway {gw}, {verdict}");
            state.applied_trust = Some(trusted);
        }
        Err(e) => eprintln!("tailcloak: failed to toggle tailscale: {e}"),
    }
}
