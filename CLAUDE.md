# tailcloak

A lightweight, native macOS background daemon in Rust that toggles Tailscale based on a trusted-list of **gateway MAC addresses**. Keeps the machine protected on untrusted networks. Built as a Rust learning exercise.

**Core logic:** current gateway MAC ∈ trusted set → `tailscale down`; else → `tailscale up`.

**Working style (for future sessions):** learning is prioritized over speed — I write code myself with guidance; sketch shapes and explain _why_ rather than handing over finished blocks. Exception: pure plumbing I've explicitly delegated (e.g. the launchd / crates.io setup).

## Status

**Complete and ready to publish.** Event-driven daemon + launchd deployment + crates.io packaging (dual MIT/Apache-2.0) all done. `cargo clippy` + `cargo test` (18 tests) green. Env: Rust stable, aarch64-apple-darwin.

Built incrementally: config → tailscale subprocess → gateway detection → CLI → event-driven daemon → launchd deploy → crates.io packaging → publish polish (`service` namespacing, `pause`/`resume`, `--help`/`--version`).

Possible follow-ups (not started): dedupe redundant `tailscale up/down` calls (a `tailscale status --json` guard), Homebrew tap, optional menu-bar UI, a `CFRunLoopTimer` to resume the daemon at pause-expiry without waiting for the next network change.

## Architecture

- `src/main.rs` — argv dispatch. `None` → `daemon::run()`; `run-once` → `daemon::run_once()`; `trust-current`/`distrust-current`/`show-trusted`/`pause`/`resume`/`service` → `cmd_*` (cross-module orchestrators); `service <install|uninstall>` is dispatched on `nth(2)` inside `cmd_service`; `pause 0` delegates to `cmd_resume`; `--help`/`-h`/`help` → `print_usage()`; `--version`/`-V` → `CARGO_PKG_VERSION`; unknown → exit 2. Non-macOS builds fail via `compile_error!`.
- `src/daemon.rs` — SCDynamicStore watches `State:/Network/Global/IPv4`+`IPv6`; CFRunLoop drives a level-triggered `reconcile()` (pause guard → load config → resolve gateway → toggle Tailscale), log-and-continue.
- `src/network.rs` — `pub use netdev::MacAddr`; `current_mac_gateway()` → pure `physical_gateway_mac(&[Interface])`.
- `src/tailscale.rs` — `resolve()` (absolute path from standard macOS locations) + `up()`/`down()`.
- `src/config.rs` — `Config { trusted_gateway_macs: HashSet<MacAddr> }`; `load_or_default`, atomic `save`, `add/remove_trusted_gateway`, `show_trusted`; `pub config_dir()` + `pub state_dir()` over a shared `xdg_dir(var, fallback)` helper. Private `load_from`/`save_to(&Path)` test seams.
- `src/pause.rs` — temporary-override window. `set`/`clear`/`remaining` over a `pause-until` Unix-timestamp file in `state_dir()` (ephemeral runtime state, not config); reads **fail open** (missing/expired/garbage → not paused). Pure `set_at`/`clear_at`/`remaining_at(&Path, SystemTime)` test seams.
- `src/launchd.rs` — `LABEL` const (`dev.fmeier.tailcloak`; single source of truth for plist Label, filename, store session); `install`/`uninstall` (driven via `service` subcommand); pure `plist_contents()`.

**Deps:** `netdev` (serde), `serde`, `toml`, `dirs`, `system-configuration`, `core-foundation`; `tempfile` (dev).

**Config** (`~/.config/tailcloak/config.toml`, outside the repo):

```toml
trusted_gateway_macs = ["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"]
```

## Key decisions (don't relitigate)

- **Trust by gateway MAC, not SSID** — harder to spoof, stable across rebrands, no Location Services prompt. Apple redacts SSIDs in the terminal.
- **`MacAddr` = `netdev::MacAddr`** (`pub use`) — `[u8;6]`-backed, free FromStr/Display/Hash/Eq/serde.
- **Per-interface gateway, never the global default route** — a full-tunnel VPN hijacks the global route to `utun` (zero MAC); physical interfaces keep their DHCP gateway. Selector drops virtual interfaces and requires a unicast, non-zero MAC.
- **Daemon is level-triggered + self-healing** — `reconcile()` re-asserts desired state on every change and ignores manual overrides; that's the security guarantee.
- **`system-configuration` + `core-foundation` crates, not hand-rolled FFI** — CFRunLoop is single-threaded, so no `Arc`/`Mutex`.
- **Missing config tolerated** → empty trusted set → always `tailscale up` (paranoid default). Config reloaded each event.
- **`HashSet<MacAddr>`** for set semantics. **Manual XDG resolution** (`$XDG_CONFIG_HOME` → `$HOME/.config` for config; `$XDG_STATE_HOME` → `$HOME/.local/state` for the ephemeral pause file), since `dirs::config_dir()` returns `~/Library/Application Support`. Both anchor to `$HOME` so the path matches between the CLI and the launchd daemon — not `$TMPDIR`, which launchd may resolve differently and break the pause IPC.
- **Orchestrators (`cmd_*`) in `main.rs`; building blocks in domain modules.** Extract to `src/cli.rs` only if `main.rs` outgrows it.
- **LaunchAgent, not LaunchDaemon** — runs as the user at login; nothing to protect pre-login.
- **`KeepAlive = { SuccessfulExit: false }`** — restart on crash; `launchctl bootout` is the hard stop (don't `kill` the PID, launchd restarts it). No signal handler needed. For a *temporary* override use `pause`, not `bootout`.
- **`service install`/`service uninstall` namespaced, not top-level** — bare `install`/`uninstall` collides with package-manager semantics (`cargo install`/`uninstall` own the binary). The `service` prefix scopes them to the launchd agent. `cmd_service` dispatches on `nth(2)`; only `install`/`uninstall` exist under it.
- **`pause` is filesystem IPC, not signals** — the CLI process and the daemon share only the FS, so `pause` writes a `pause-until` timestamp the daemon reads at the top of each `reconcile()`. `cmd_pause` also runs `tailscale down` immediately (pause is only useful paired with a takedown — e.g. authenticating on a captive network behind an exit node); `cmd_resume` clears the file and reconciles now so protection returns without waiting for a network change. Bounded + auto-resuming (vs. `bootout`, which is indefinite and leaves you unprotected). Reads fail open. No active timer yet → if left to lapse, resumes on the first network change after expiry (acceptable; timer is a noted follow-up). Deliberately *not* polled — a repeating timer would violate the event-driven, no-polling design.
- **`service install` is self-contained** — generates the plist from `current_exe()`, no template/symlink, idempotent. Deploy: `cargo install` then `tailcloak service install`.
- **Daemon resolves `tailscale` to an absolute path; plist carries no `PATH`** — keeps Tailscale-location knowledge in `tailscale.rs`. `install` reuses `resolve()` for a non-blocking "not installed" warning.

## Known hazards

- **`tailscale up` needs connectivity to authenticate** — slow on untrusted networks; calls are redundant per event (a `status --json` guard would dedupe).
- **No gateway = untrusted** → losing the network brings Tailscale up. Intended.
- **An installed agent embeds an absolute binary path** — `cargo clean` or moving the repo breaks a dev install; re-run `tailcloak service install`.

## Develop & release

- Test / lint: `cargo test`, `cargo clippy --all-targets`.
- Local service: `tailcloak service install` / `service uninstall`; temporary override with `tailcloak pause <min>` / `resume`; logs at `~/Library/Logs/tailcloak.log`; inspect with `launchctl print gui/$(id -u)/dev.fmeier.tailcloak`.
- Publish: dual-licensed `MIT OR Apache-2.0`; `cargo publish` (versions are immutable — only yankable). Tag releases for crates.io / a future Homebrew tap.
- Commits: `feat|fix|refactor|chore|test|docs(scope): …`; scope mirrors the module (`config`, `network`, `tailscale`, `daemon`, `launchd`, `pause`, `main`).
