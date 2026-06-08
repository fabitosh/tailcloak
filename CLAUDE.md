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

### Phase 4: macOS SystemConfiguration — Gateway MAC Query

Goal: replace the `current_mac_gateway()` stub with a real lookup. Two-step problem:

1. **Find the default gateway's IP + interface.** Use `SCDynamicStoreCopyValue` on key `State:/Network/Global/IPv4` → `CFDictionary` with `Router` (IP string) + `PrimaryInterface` (e.g. `en0`).
2. **Resolve gateway IP → MAC via ARP.** Pragmatic path: shell out to `arp -n <ip>` and parse. Deeper FFI path (optional, later): `sysctl(NET_RT_FLAGS)` + walk `rt_msghdr` to read the kernel ARP table directly.

- Add crates: `core-foundation`, `system-configuration`
- **First exercise:** throwaway `examples/print_gateway.rs` that prints `(interface, gateway_ip)` before integrating. Lets you isolate the FFI learning.
- **Concepts:** `unsafe` blocks, FFI, raw pointers, CoreFoundation's retain/release model vs Rust ownership, `CFDictionary` → `CFString` extraction, parsing subprocess output
- **Learning goal:** how Rust exposes C APIs; when `unsafe` is necessary; reading crate docs to map thin wrappers around C types

### Phase 5: CLI Subcommands (config management)

Goal: stop hand-editing `~/.config/tailcloak/config.toml`. Expose a CLI surface so the user can trust the current network from any terminal in one command.

- **Subcommands (planned):**
  - `tailcloak` (no args) → run the daemon logic (current behavior)
  - `tailcloak trust-current` → add current gateway MAC to config
  - `tailcloak show-trusted` → print the trusted list (debug aid; may grow to print current gateway too)
- **Argv dispatch in `main.rs`** via `std::env::args().nth(1).as_deref()` + `match`. Skeleton already in place with `todo!()` stubs.
- **Building blocks to add in `src/config.rs`:**
  - `impl Serialize for Config` + on `MacAddr` (currently `Deserialize`-only)
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

Last working session ended at the boundary of Phase 3 → Phase 4, after pivoting from SSID-based to gateway-MAC-based trust.

### Current files

- `src/main.rs` — returns `Result<_, Box<dyn Error>>`; argv dispatch via `match std::env::args().nth(1).as_deref()` with arms for `None` → `run_daemon_once()`, `trust-current` → `cmd_trust_current()`, `show-trusted` → `cmd_show_trusted()`, unknown → `eprintln + exit(2)`. Daemon path (`run_daemon_once`) computes `is_trusted` from `current_mac_gateway()` + `config.trusted_gateway_macs`. **`cmd_*` functions are `todo!()` stubs** awaiting Phase 5.
- `src/config.rs` — loads `~/.config/tailcloak/config.toml` (XDG-aware) into `Config { trusted_gateway_macs: HashSet<MacAddr> }`
- `src/tailscale.rs` — `up()` / `down()` shelling out via `std::process::Command`, checking `ExitStatus::success()`
- `src/network.rs` (was `wifi.rs`) — `MacAddr` newtype with `FromStr` validator (colon notation only, rejects dash notation explicitly); `current_mac_gateway() -> Option<MacAddr>` is a **stub** returning a hardcoded MAC, to be replaced in Phase 4

### Config file format (lives at `~/.config/tailcloak/config.toml`, outside the repo)

```toml
trusted_gateway_macs = ["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"]
```

Renamed from `trusted_ssids` during the pivot — if you have an older config file, the TOML key needs updating too.

### Key decisions made (don't relitigate)

- **Trust identifier is the default gateway's MAC**, not SSID. Reasons: (a) MAC is harder to spoof than an SSID string, (b) stable across SSID rebrands within the same physical network, (c) avoids macOS Location Services permission prompt that recent macOS versions require for SSID reads via `CWWiFiClient`. Tradeoff: doesn't disambiguate Ethernet from Wi-Fi on the same gateway — fine for the threat model (the gateway *is* the network's identity).
- **`MacAddr` is a newtype around `String`**, not `[u8; 6]`. Reason: keeps `FromStr` + `serde::Deserialize` simple, and the value is only ever compared for equality. If we ever need to render canonical form or compare with another representation, switch to `[u8; 6]`.
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

- **macOS `arp` strips leading zeros from each octet.** Output looks like `f8:d:a9:c8:4:4`, the canonical form is `f8:0d:a9:c8:04:04`. The current `MacAddr::from_str` requires `p.len() == 2` per octet, so feeding `arp` output directly to it will **always fail validation**. Phase 4 parser must pre-pad each octet via `format!("{:02x}", u8::from_str_radix(p, 16)?)` *before* `parse::<MacAddr>()`. Strict validation stays in the type; normalization happens at the I/O boundary.
- **`tailscale up` requires network connectivity to authenticate.** Phase 4 will trigger this on untrusted networks where DNS/auth may be slow or rate-limited. If it becomes noisy, consider passing `--accept-routes=...` or similar to make the call cheaper, or add the `is_tailscale_up` guard we previously rejected.
- **`current_mac_gateway()` returning `None` is interpreted as "untrusted"** in `run_daemon_once`. Means: lose Wi-Fi entirely → daemon tries `tailscale up`. Probably desired (paranoid default) but flag this if it ever causes surprises.

### Next session start: continue Phase 4 / start Phase 5 prep

Phase 4 is the next functional milestone — Phase 5's `cmd_*` stubs do nothing useful until `current_mac_gateway()` returns a real value.

**Phase 4 plan:**

1. **Crate selection:** `core-foundation` + `system-configuration` for the SCDynamicStore call. Confirm latest versions and that the public API covers `SCDynamicStoreCopyValue` returning a `CFDictionary`.
2. **Two-step flow:**
   - Step 1 — gateway IP + interface: `SCDynamicStoreCopyValue("State:/Network/Global/IPv4")` → dict with `Router` (string IP) + `PrimaryInterface` (e.g. `en0`).
   - Step 2 — IP → MAC: shell out to `arp -n <ip>` and parse the MAC out (remember the leading-zero pad — see Hazards). Deep FFI option for later: `sysctl(NET_RT_FLAGS)` + `rt_msghdr` walk.
3. **First exercise:** throwaway `examples/print_gateway.rs` that prints `(interface, gateway_ip, gateway_mac)`. Isolate the FFI + subprocess parsing before wiring into `main`.
4. **Edge cases to handle:** no default gateway (no Wi-Fi, no Ethernet) → `None`; ARP entry missing/incomplete (`(incomplete)` in `arp` output) → `None`; multiple default routes (rare on macOS) → first one wins.

**Phase 5 can start in parallel** with skeleton work — the building blocks in `config.rs` (`Serialize`, `load_or_default`, `save`, `add_gateway`) don't depend on Phase 4 and would let you test the read-modify-write cycle against a hand-edited config first. Then `cmd_trust_current` lights up the moment Phase 4 lands.

After both, Phase 6 wires `SCDynamicStoreSetNotificationKeys` + `CFRunLoop` to make it event-driven (watching `State:/Network/Global/IPv4` for changes).
