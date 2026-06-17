# tailcloak

A lightweight, native macOS background daemon in Rust that toggles Tailscale up/down based on a trusted-list of **default-gateway MAC addresses** (not SSID — see Key Decisions). Motivation: keep my machine safe on public networks. It's a Rust learning exercise; the repo is **public**, so network identifiers live in config *outside* the repo.

**Core logic:** current gateway MAC ∈ trusted set → `tailscale down`; else → `tailscale up`.

**Constraints:** event-driven (no polling), native SystemConfiguration APIs, Tailscale CLI via `std::process::Command`, runs headless. **Learning is prioritized over speed — I write code myself with guidance; sketch shapes and explain *why* rather than handing over finished blocks (unless I ask).**

---

## Status

Phases 0–6 complete. **Next: Phase 7 (launchd deployment).** Tree builds clean; `cargo clippy` + `cargo test` (12 tests) green.

Env: Rust 1.96.0 stable (aarch64-apple-darwin), rust-analyzer via `rustup component`, Neovim + LazyVim Rust extra.

## Curriculum (✅ = done)

- **0 — Dev env** ✅ rustup, LazyVim Rust extra, rust-analyzer.
- **1 — Bootstrap** ✅ `cargo init`, edition 2024, authors `["fabitosh"]`.
- **2 — Config** ✅ `Config { trusted_gateway_macs: HashSet<MacAddr> }` from `~/.config/tailcloak/config.toml`; serde + toml + dirs; manual XDG resolution.
- **3 — Tailscale subprocess** ✅ `tailscale::up()/down()` checking `ExitStatus::success()`.
- **4 — Gateway MAC via `netdev`** ✅ pure `physical_gateway_mac(&[Interface])` selector (6 unit tests); reads each *physical* interface's own gateway (VPN-invariant), not the global default route.
- **5 — CLI subcommands** ✅ `trust-current`, `distrust-current`, `show-trusted`, `run-once`; `Config::{load_or_default, save (atomic), add/remove_trusted_gateway, show_trusted}` (6 tests, `tempfile`).
- **6 — Event-driven daemon** ✅ `src/daemon.rs`: SCDynamicStore watches `State:/Network/Global/IPv4`+`IPv6`, CFRunLoop drives a level-triggered `reconcile()`; via `system-configuration` + `core-foundation` crates (no raw FFI).
- **7 — launchd deployment** ⬜ next (see bottom).

## Current files

- `src/main.rs` — argv dispatch (`std::env::args().nth(1).as_deref()`): `None` → `daemon::run()`, `run-once` → `daemon::run_once()`, `trust-current`/`distrust-current`/`show-trusted` → `cmd_*`, unknown → `eprintln + exit(2)`. `cmd_*` are orchestrators.
- `src/daemon.rs` — SCDynamicStore + CFRunLoop setup; `reconcile()` (load config → resolve gateway → toggle Tailscale), log-and-continue.
- `src/config.rs` — `Config` (derives `Serialize/Deserialize/Default/PartialEq`); `load_or_default`, atomic `save`, `add/remove_trusted_gateway`, `show_trusted`. Private `load_from`/`save_to(&Path)` seams for tests.
- `src/network.rs` — `pub use netdev::MacAddr`; `current_mac_gateway()` → pure `physical_gateway_mac(&[Interface])`.
- `src/tailscale.rs` — `up()` / `down()` via `std::process::Command`.
- `examples/probe_netdev.rs` — throwaway per-interface gateway dump (untracked).

**Deps:** `netdev` (serde feature), `serde`, `toml`, `dirs`, `system-configuration` 0.7, `core-foundation` 0.9; `tempfile` (dev).

**Config format** (`~/.config/tailcloak/config.toml`, outside the repo):
```toml
trusted_gateway_macs = ["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"]
```

## Key decisions (don't relitigate)

- **Trust by default-gateway MAC, not SSID.** Harder to spoof, stable across SSID rebrands, avoids the Location Services prompt for SSID reads. Tradeoff: doesn't distinguish Ethernet vs Wi-Fi on one gateway — fine (the gateway *is* the network's identity).
- **`MacAddr` = `netdev::MacAddr`** (re-exported via `pub use`), `[u8;6]`-backed — gives FromStr/Display/Hash/Eq/serde for free. Superseded an earlier `MacAddr(String)` newtype; lost strict colon-only validation, acceptable since MACs come from a typed source.
- **Read per-interface gateway, never `netdev::get_default_gateway()`.** A full-tunnel VPN (Tailscale exit node) hijacks the global default route to the `utun` tunnel (all-zero MAC). Each physical interface keeps its DHCP gateway regardless of VPN state. Selector drops virtual interfaces (`Tunnel|Loopback|PeerToPeerWireless`) and requires a unicast, non-zero gateway MAC.
- **Daemon is level-triggered + self-healing.** `reconcile()` runs on every watched-key change and asserts the desired Tailscale state. It does **not** respect manual overrides: if Tailscale drops on an untrusted network it gets re-asserted up — that's the security guarantee. (An earlier `applied_trust` edge-trigger guard was removed because it tracked our *intent*, not Tailscale's actual state, so it failed to self-heal.)
- **Daemon via `system-configuration` + `core-foundation` crates**, not hand-rolled FFI. The crate heap-boxes the callback context (owned by the store) so it outlives the run loop; CFRunLoop is single-threaded → no `Arc`/`Mutex`.
- **`load_or_default` tolerates a missing config** (`io::ErrorKind::NotFound` → empty set → always `tailscale up`, the paranoid default). `reconcile` reloads config each event, so edits apply on the next network change (not instantly).
- **CLI orchestrators (`cmd_*`) stay in `main.rs`; thin building blocks live in domain modules** (`Config::add/remove_trusted_gateway`, `Config::save`). Don't push orchestration into domain modules. Extract to `src/cli.rs` if `main.rs` outgrows it.
- **`HashSet<MacAddr>`** not `Vec` (set semantics; `.contains(&MacAddr)` via `Borrow`).
- **Manual XDG resolution** (`$XDG_CONFIG_HOME` → `$HOME/.config`), not `dirs::config_dir()` (which returns `~/Library/Application Support`).
- **Config lives outside the repo** (private dotfiles / 1Password). Public dotfiles will ship only a plist template + setup script.

## Known hazards

- **`tailscale up` needs connectivity to authenticate** — slow/noisy on untrusted networks. The daemon re-runs up/down on every watched-key event (no actual-state guard), so calls/logs are redundant; a `tailscale status --json` (`BackendState`) check would dedupe if it gets noisy.
- **`current_mac_gateway() == None` is treated as untrusted** → losing the network triggers `tailscale up`. Paranoid default; intended.

## Commit conventions

`feat|fix|refactor|chore|test(scope): …`; scope mirrors the module (`config`, `network`, `tailscale`, `daemon`, `main`).

## Next: Phase 7 — launchd deployment

Make the headless binary a proper macOS background service.

- `launchd` plist at `~/Library/LaunchAgents/com.fabitosh.tailcloak.plist`; `RunAtLoad = true`; stdout/stderr → `~/Library/Logs/tailcloak.log`.
- `launchctl bootstrap gui/$UID <plist>` / `launchctl bootout` workflow.
- Binary: `cargo build --release`, symlink to `~/.local/bin/tailcloak`.
- Public dotfiles ship a plist *template* (placeholder paths/MACs) + setup script; real config stays private.
- **Concepts:** launchd vs systemd, `RunAtLoad`/`KeepAlive`, signal handling / graceful shutdown.
