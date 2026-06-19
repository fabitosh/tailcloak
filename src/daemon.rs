//! Event-driven daemon acting on physical network changes.

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
    let config = match Config::load_or_default() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("tailcloak: failed to load config: {e}");
            return;
        }
    };

    let gateway = network::current_mac_gateway();
    let trusted = gateway.is_some_and(|mac| config.trusted_gateway_macs.contains(&mac));
    let result = if trusted {
        tailscale::down()
    } else {
        tailscale::up()
    };
    let gw = gateway.map_or_else(|| "none".to_string(), |mac| mac.to_string());
    match result {
        Ok(()) => println!(
            "tailcloak: gateway {gw} {} -> tailscale {}",
            if trusted { "trusted" } else { "untrusted" },
            if trusted { "down" } else { "up" }
        ),
        Err(e) => eprintln!("tailcloak: failed to toggle tailscale: {e}"),
    }
}
