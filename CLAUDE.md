Context: Looking to implement this specific project in Rust as a learning exercise on macOS.

It's primary motivation is to keep my machine save in public networks.

The final script will reside in my public dotfiles. I do not want network identifiers public. find a way to have those as general configuration not shared publicly.

Task: Build a lightweight, native macOS background daemon in Rust that automatically toggles Tailscale up or down based on an exclude-list (trusted list) of network identifiers. Identification is by **default-gateway MAC address**, not SSID — see "Key decisions" below for the why.

Core Logic idea:

• If current gateway MAC is in TRUSTED_GATEWAY_MACS  run Tailscale down

• Else run Tailscale up

High-Level Implementation Thoughts:

1. **Architecture:** Avoid low-order polling loops (sleep()). The daemon should ideally be event-driven by subscribing to network state changes using native macOS APIs.
2. **macOS Integration:** Interface with Apple's SystemConfiguration framework.
3. **Subprocesses:** Use std::process::Command to robustly invoke the Tailscale CLI.
4. **Deployment:** The final binary should be designed to run headlessly

Please help me bootstrap this project.

Provide a clean Cargo.toml, guide me through accessing the macOS SystemConfiguration API in Rust to fetch the SSID or listen to network events, and write the core event-loop architecture. Speed in implementation is not the primary goal, prioritize my learning. I don't mind writing code myself as part of that journey.

---

## Curriculum / Build Plan

Each phase has a clear learning goal. Work phases sequentially — later phases assume the concepts from earlier ones.

### Phase 0: Dev Environment ✅

- [x] rustup + stable toolchain (1.96.0, aarch64-apple-darwin)
- [x] LazyVim Rust extra (`lang.rust` in `~/.config/nvim/lazyvim.json`) — ships `rustaceanvim`, `nvim-dap`, rustfmt
- [x] rust-analyzer component installed via `rustup component add rust-analyzer` (was a stale cargo-bin proxy)
- [ ] Key cargo commands to internalize: `cargo check` (fast type-check), `cargo clippy` (linter), `cargo fmt`, `cargo doc --open`
- **Learning goal:** understand Cargo as both build system and package manager; get the edit→check loop fluid in LazyVim before writing real logic

### Phase 1: Project Bootstrap ✅

- [x] `cargo init` inside the existing directory (not `cargo new`)
- [x] `Cargo.toml`: name, version, `edition = "2024"`, authors = `["fabitosh"]`
- **Concepts:** crate vs package, `Cargo.lock` (commit it for binaries), editions, `src/main.rs` entry point
- **Learning goal:** Cargo project anatomy; how `use`, `mod`, and file layout relate

### Phase 2: Configuration System (Private Network Identifiers) ✅

- [x] Config at `~/.config/tailcloak/config.toml` — never inside the repo
- [x] Deps: `serde` (derive), `toml`, `dirs`
- [x] `Config` struct with `trusted_gateway_macs: HashSet<MacAddr>` (set semantics, cleaner `.contains(&MacAddr)` via `Borrow`). Originally `HashSet<String>` named `trusted_ssids`; renamed/retyped after the gateway-MAC pivot.
- [x] Proper XDG resolution: check `$XDG_CONFIG_HOME` first, fall back to `$HOME/.config` (not `dirs::config_dir()` which returns `~/Library/Application Support` on macOS)
- [x] Errors propagated via `?` and `Box<dyn std::error::Error>`
- **Concepts:** structs, `impl` blocks, `derive` macros, `Result<T, E>`, the `?` operator, file I/O with `std::fs`
- **Learning goal:** Rust's error handling model; how `serde` + `toml` turn a file into a typed struct

### Phase 3: Core Logic + Tailscale Subprocess ✅

- [x] `src/tailscale.rs` with `pub fn up()` / `pub fn down()` — both return `Result<(), Box<dyn Error>>`, check exit status with `status.success()`
- [x] `src/network.rs` (was `wifi.rs`): `MacAddr` newtype with `FromStr` parse + validation; stub `current_mac_gateway() -> Option<MacAddr>` returning hardcoded value
- [x] `main.rs` returns `Result<(), Box<dyn Error>>`, branches on `current_gateway.is_some_and(|m| config.trusted_gateway_macs.contains(&m))`
- [x] No `is_tailscale_up` guard — `tailscale up`/`down` are fast and idempotent enough; revisit only if logs get noisy
- **Concepts:** modules and visibility (`pub`, `mod`), `std::process::Command`, `String` vs `&str`, `Option<T>`, pattern matching (`match`, `if let`), `Option::is_some_and`, newtype pattern, `FromStr` trait
- **Learning goal:** Rust's module system; ownership distinction between owned String and borrowed &str; newtypes for domain-meaningful validation

### Phase 4: Gateway MAC Query (via `netdev`) ✅

Goal: replace the `current_mac_gateway()` stub with a real lookup. **Pivoted away from the original SystemConfiguration + `arp` FFI plan** to the `netdev` crate, which reads interfaces/gateways cross-platform (and internally uses SystemConfiguration on macOS — see its `apple-system-configuration-extra` default feature, relevant for Phase 6).

- [x] Dep: `netdev = { version = "0.44.0", features = ["serde"] }`. The `serde` feature provides `Serialize`/`Deserialize` on `netdev::MacAddr` (via the `mac-addr` crate) — needed for the config `HashSet`.
- [x] `current_mac_gateway()` → `physical_gateway_mac(&netdev::get_interfaces())`, a pure, unit-tested selector.
- [x] **Does NOT use `netdev::get_default_gateway()`.** A full-tunnel VPN (Tailscale exit node) becomes the *primary* network service, so the global default route points at the `utun` tunnel — no L2 gateway → all-zero MAC. Instead we iterate interfaces and read each *physical* interface's own `.gateway`, which is VPN-invariant.
- [x] Selection logic: drop virtual interfaces (deny-list: `Tunnel | Loopback | PeerToPeerWireless`), then require a gateway whose MAC `is_unicast()` and `!= MacAddr::zero()` (netdev returns zero for unresolved/tunnel gateways).
- [x] 6 unit tests over the pure `physical_gateway_mac(&[Interface])` seam, using `Interface::dummy()` fixtures (no system access).
- [x] Throwaway `examples/probe_netdev.rs` dumps per-interface gateways — the before/after `tailscale down` invariance check.
- **Concepts learned:** L2 vs L3 (MAC is per-hop/link-local, IP is end-to-end); the ARP cache & how `netstat -rn` folds it in (`L` flag); default route vs per-interface route; primary network service / VPN default-route hijack; pure-function test seams for system-dependent code.
- **Learning goal (revised):** reading a crate's source to map its API + features; isolating impure system calls behind a testable pure core.

### Phase 5: CLI Subcommands (config management)

Goal: stop hand-editing `~/.config/tailcloak/config.toml`. Expose a CLI surface so the user can trust the current network from any terminal in one command.

- **Subcommands (planned):**
  - `tailcloak` (no args) → run the daemon logic (current behavior)
  - `tailcloak trust-current` → add current gateway MAC to config
  - `tailcloak show-trusted` → print the trusted list (debug aid; may grow to print current gateway too)
- **Argv dispatch in `main.rs`** via `std::env::args().nth(1).as_deref()` + `match`. Skeleton already in place with `todo!()` stubs.
- **Building blocks to add in `src/config.rs`:**
  - `#[derive(Serialize)]` on `Config` (currently `Deserialize`-only). `MacAddr` already has `Serialize` from netdev's `serde` feature — no work there.
  - `Config::load_or_default()` → maps `io::ErrorKind::NotFound` to `Default::default()`, propagates anything else. Needed because `trust-current` is precisely the command users run *before* a config exists.
  - `Config::save(&self) -> Result<()>` → atomic-rename pattern (`.tmp` + `fs::rename`) to avoid half-written configs on crash. Creates parent dir with `fs::create_dir_all` if missing.
  - `Config::add_gateway(&mut self, mac: MacAddr) -> bool` → mirrors `HashSet::insert` semantics so the CLI can print "trusted" vs "already trusted".
- **Concepts:** subcommand dispatch without a CLI framework, atomic file writes, `Default` trait, the symmetry of `Serialize`/`Deserialize`
- **Learning goal:** how to grow a binary's surface area without prematurely reaching for `clap`; thinking about the orchestration layer vs the domain modules

### Phase 6: Event-Driven Network Monitoring

- Replace any remaining poll logic with `SCDynamicStoreSetNotificationKeys`
- Register a callback that fires on changes to `State:/Network/Global/IPv4` (network switches, gateway changes)
- Run the `CFRunLoop` on the main thread; trigger core logic from the callback
- **Concepts:** `extern "C" fn` callbacks, function pointers vs closures, lifetimes (why the callback context must outlive the run loop), `Arc`/`Mutex` for shared state if threading is introduced
- **Learning goal:** event-driven architecture in Rust; the lifetime system in a concrete, non-trivial case

### Phase 7: Daemon Deployment

- Write a `launchd` plist at `~/Library/LaunchAgents/com.fabitosh.tailcloak.plist`
- Redirect stdout/stderr to `~/Library/Logs/tailcloak.log`
- `launchctl bootstrap` / `launchctl bootout` workflow; `RunAtLoad = true`
- **Concepts:** macOS daemon model (launchd vs systemd), signal handling basics, graceful shutdown
- **Learning goal:** how a headless Rust binary becomes a proper macOS background service

### Dotfiles Strategy

- Binary: build with `cargo build --release`, symlink to `~/.local/bin/tailcloak`
- Config: `~/.config/tailcloak/config.toml` — tracked in a **private** dotfiles repo or 1Password; never committed here
- Public dotfiles: ship only the launchd plist template (placeholder MACs) and a setup script
- This repo ships code only; secrets live outside it

### Environment Status

- Rust: 1.96.0 stable, aarch64-apple-darwin ✅
- rust-analyzer: 1.96.0 via `rustup component` ✅
- Neovim: 0.12.2 + LazyVim Rust extra ✅
- Cargo project: initialized, committed, pushed to GitHub ✅

---

## Session State (resume point)

Last working session completed Phase 4 (real gateway MAC via `netdev`) and the `MacAddr` → `netdev::MacAddr` pivot. **Next up: Phase 5 (CLI subcommands).**

### Current files

- `src/main.rs` — returns `Result<_, Box<dyn Error>>`; argv dispatch via `match std::env::args().nth(1).as_deref()` with arms for `None` → `run_daemon_once()`, `trust-current` → `cmd_trust_current()`, `show-trusted` → `cmd_show_trusted()`, unknown → `eprintln + exit(2)`. `run_daemon_once` resolves the real gateway, prints it via `Display`, and computes `is_trusted`. **`cmd_*` functions are `todo!()` stubs** awaiting Phase 5.
- `src/config.rs` — loads `~/.config/tailcloak/config.toml` (XDG-aware) into `Config { trusted_gateway_macs: HashSet<MacAddr> }`. Derives `Deserialize` only (Phase 5 adds `Serialize` + `save`/`load_or_default`/`add_gateway`).
- `src/tailscale.rs` — `up()` / `down()` shelling out via `std::process::Command`, checking `ExitStatus::success()`
- `src/network.rs` (was `wifi.rs`) — `pub use netdev::MacAddr`; **`current_mac_gateway()` is real**, delegating to a pure `physical_gateway_mac(&[Interface])` (physical-only, unicast non-zero gateway MAC) with 6 unit tests.
- `examples/probe_netdev.rs` — throwaway per-interface gateway dump (untracked; keep as a learning aid or delete).

### Config file format (lives at `~/.config/tailcloak/config.toml`, outside the repo)

```toml
trusted_gateway_macs = ["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"]
```

Renamed from `trusted_ssids` during the pivot — if you have an older config file, the TOML key needs updating too.

### Key decisions made (don't relitigate)

- **Trust identifier is the default gateway's MAC**, not SSID. Reasons: (a) MAC is harder to spoof than an SSID string, (b) stable across SSID rebrands within the same physical network, (c) avoids macOS Location Services permission prompt that recent macOS versions require for SSID reads via `CWWiFiClient`. Tradeoff: doesn't disambiguate Ethernet from Wi-Fi on the same gateway — fine for the threat model (the gateway *is* the network's identity).
- **`MacAddr` is `netdev::MacAddr`** (re-exported as `crate::network::MacAddr` via `pub use`), a `[u8; 6]`-backed type. **Superseded the earlier `MacAddr(String)` newtype** once `netdev` entered: netdev's type already derives `FromStr`, `Display`, `Hash`, `Eq`, and — with the `serde` feature — `Serialize`/`Deserialize`, so a wrapper added nothing. Tradeoff: lost the strict colon-only / 2-char-padded validation and the custom "dash notation unsupported" error; netdev's parser is more lenient. Acceptable because MACs now come from a trusted typed source (netdev), not just hand-edited config. (Original rationale for String-backing — "keeps FromStr + serde simple" — predated pulling in netdev, which now gives both for free.)
- **`trusted_gateway_macs: HashSet<MacAddr>`**, not `Vec`. Reason: set semantics + `HashSet::contains` accepts `&MacAddr` via `Borrow`, cleaner than `Vec::contains`.
- **Error handling pattern**: in the current "run once" code, `main` returns `Result` and uses `?` everywhere. In Phase 5's event handlers, will switch to **log-and-continue** (`if let Err(e) = ... { eprintln!(...) }`) so one failed toggle doesn't kill the daemon.
- **No `is_tailscale_up` guard**. `tailscale up`/`down` confirmed fast on this machine; idempotent enough.
- **XDG path resolution is manual** (not via `dirs::config_dir()`) because `dirs` returns the macOS-native `~/Library/Application Support`, and we want `~/.config` (or whatever `$XDG_CONFIG_HOME` says) to match chezmoi-managed dotfiles.
- **Config file lives outside the repo.** Tracked separately in private dotfiles. Public dotfiles will eventually ship only a launchd plist template + setup script.
- **CLI subcommands live in `main.rs` for now.** When `main.rs` grows past ~150 lines, a 4th command gets added, or a `cmd_*` function needs unit testing in isolation, extract to `src/cli.rs`. Do **not** scatter `cmd_*` into the domain modules (`config`, `network`) — they're orchestrators that cross domain boundaries; sticking them in a domain module would couple that module to the others. Reverse direction is fine: the orchestrators call thin building-block methods that live in the domain modules (e.g. `Config::add_gateway`, `Config::save`).
- **`run_daemon_once` currently uses `.expect(...)` on `Config::load`.** Acceptable for the daemon path, but `cmd_trust_current` cannot use the same pattern — its whole purpose is bootstrapping a missing config. Implementation will gate this with `Config::load_or_default()` (see Phase 5). Worth revisiting whether the daemon should also tolerate missing config (empty trusted set → always `tailscale up`).

### Commit conventions in use

- `feat(scope): ...` — new capability
- `fix(scope): ...` — corrects wrong behavior (e.g., silent error swallowing)
- `refactor(scope): ...` — pure code change, no behavior delta
- `chore(deps): ...` — dependency bumps
- Scope mirrors module name (`config`, `tailscale`, `network`, `main`)

### Known hazards / gotchas (don't get bitten again)

- ~~**macOS `arp` strips leading zeros from each octet.**~~ **MOOT (Phase 4 used netdev).** We never shell out to `arp`; `netdev` returns `[u8; 6]` and formats via `{:02x}` (always zero-padded). Kept only so the lesson isn't re-learned: this hazard existed solely for the abandoned `arp`-parsing path.
- **`tailscale up` requires network connectivity to authenticate.** Phase 4 will trigger this on untrusted networks where DNS/auth may be slow or rate-limited. If it becomes noisy, consider passing `--accept-routes=...` or similar to make the call cheaper, or add the `is_tailscale_up` guard we previously rejected.
- **`current_mac_gateway()` returning `None` is interpreted as "untrusted"** in `run_daemon_once`. Means: lose Wi-Fi entirely → daemon tries `tailscale up`. Probably desired (paranoid default) but flag this if it ever causes surprises.

### Next session start: Phase 5 (CLI subcommands)

Phase 4 is done, so the `cmd_*` stubs can now light up. Build order (config building blocks first, then wire the orchestrators):

1. **`config.rs` building blocks** (the thin, testable domain methods):
   - `#[derive(Serialize)]` on `Config` (MacAddr's `Serialize` already comes from netdev).
   - `Config::load_or_default()` — map `io::ErrorKind::NotFound` → `Config::default()`, propagate everything else. Needed because `trust-current` runs *before* a config exists. (`#[derive(Default)]` on `Config`.)
   - `Config::save(&self)` — atomic write: serialize to a `.tmp` sibling, then `fs::rename`. `fs::create_dir_all` the parent first.
   - `Config::add_gateway(&mut self, mac) -> bool` — wraps `HashSet::insert` so the CLI can say "trusted" vs "already trusted".
2. **`main.rs` orchestrators** (cross-domain glue, stay out of the domain modules):
   - `cmd_trust_current` — `load_or_default` → `current_mac_gateway()` (error if `None`) → `add_gateway` → `save`; print the outcome.
   - `cmd_show_trusted` — `load_or_default` → print the set (and maybe the current gateway + whether it's trusted).
3. **Tests:** a `config.rs` round-trip test (build a `Config`, `save` to a `tempdir`, `load`, assert equality) exercises `Serialize`/`Deserialize` + the atomic write without touching the real `~/.config`.

Then Phase 6 wires `SCDynamicStoreSetNotificationKeys` + `CFRunLoop` for event-driven monitoring (watching `State:/Network/Global/IPv4`). Note netdev already links SystemConfiguration but does *not* expose change notifications — Phase 6 is still direct FFI.
