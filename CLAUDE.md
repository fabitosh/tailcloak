Context: Looking to implement this specific project in Rust as a learning exercise on macOS.

It's primary motivation is to keep my machine save in public networks.

The final script will reside in my public dotfiles. I do not want to have wifi ssids public. find a way to have those as general configuration not shared publicly.

Task: Build a lightweight, native macOS background daemon in Rust that automatically toggles Tailscale up or down based on an exclude-list (trusted list) of Wi-Fi SSIDs.

Core Logic idea:

• If current_ssid is in TRUSTED_SSIDS  run Tailscale down

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

### Phase 2: Configuration System (Private SSIDs) ✅

- [x] Config at `~/.config/tailcloak/config.toml` — never inside the repo
- [x] Deps: `serde` (derive), `toml`, `dirs`
- [x] `Config` struct with `trusted_ssids: HashSet<String>` (set semantics, cleaner `.contains(&str)` via `Borrow`)
- [x] Proper XDG resolution: check `$XDG_CONFIG_HOME` first, fall back to `$HOME/.config` (not `dirs::config_dir()` which returns `~/Library/Application Support` on macOS)
- [x] Errors propagated via `?` and `Box<dyn std::error::Error>`
- **Concepts:** structs, `impl` blocks, `derive` macros, `Result<T, E>`, the `?` operator, file I/O with `std::fs`
- **Learning goal:** Rust's error handling model; how `serde` + `toml` turn a file into a typed struct

### Phase 3: Core Logic + Tailscale Subprocess ✅

- [x] `src/tailscale.rs` with `pub fn up()` / `pub fn down()` — both return `Result<(), Box<dyn Error>>`, check exit status with `status.success()`
- [x] `src/wifi.rs` stub: `pub fn get_current_ssid() -> Option<String>` returning hardcoded value
- [x] `main.rs` returns `Result<(), Box<dyn Error>>`, branches on `current_ssid.as_deref().is_some_and(|s| config.trusted_ssids.contains(s))`
- [x] No `is_tailscale_up` guard — `tailscale up`/`down` are fast and idempotent enough; revisit only if logs get noisy
- **Concepts:** modules and visibility (`pub`, `mod`), `std::process::Command`, `String` vs `&str`, `Option<T>`, pattern matching (`match`, `if let`), `Option::as_deref`, `Option::is_some_and`
- **Learning goal:** Rust's module system; ownership distinction between owned String and borrowed &str

### Phase 4: macOS SystemConfiguration — SSID Query

- Add crates: `core-foundation`, `system-configuration`
- Implement real `get_current_ssid() -> Option<String>` using `CWWiFiClient` / `SCDynamicStoreCopyValue`
- **Concepts:** `unsafe` blocks, FFI (Foreign Function Interface), raw pointers, Rust's memory model vs C's retain/release, `Option` as a null-safe wrapper
- **Learning goal:** how Rust exposes C APIs; when and why `unsafe` is necessary; reading crate docs to understand thin wrappers around C types

### Phase 5: Event-Driven Network Monitoring

- Replace any remaining poll logic with `SCDynamicStoreSetNotificationKeys`
- Register a callback that fires on network interface changes (en0 up/down, SSID change)
- Run the `CFRunLoop` on the main thread; trigger core logic from the callback
- **Concepts:** `extern "C" fn` callbacks, function pointers vs closures, lifetimes (why the callback context must outlive the run loop), `Arc`/`Mutex` for shared state if threading is introduced
- **Learning goal:** event-driven architecture in Rust; the lifetime system in a concrete, non-trivial case

### Phase 6: Daemon Deployment

- Write a `launchd` plist at `~/Library/LaunchAgents/com.fabitosh.tailcloak.plist`
- Redirect stdout/stderr to `~/Library/Logs/tailcloak.log`
- `launchctl bootstrap` / `launchctl bootout` workflow; `RunAtLoad = true`
- **Concepts:** macOS daemon model (launchd vs systemd), signal handling basics, graceful shutdown
- **Learning goal:** how a headless Rust binary becomes a proper macOS background service

### Dotfiles Strategy

- Binary: build with `cargo build --release`, symlink to `~/.local/bin/tailcloak`
- Config: `~/.config/tailcloak/config.toml` — tracked in a **private** dotfiles repo or 1Password; never committed here
- Public dotfiles: ship only the launchd plist template (placeholder SSIDs) and a setup script
- This repo ships code only; secrets live outside it

### Environment Status

- Rust: 1.96.0 stable, aarch64-apple-darwin ✅
- rust-analyzer: 1.96.0 via `rustup component` ✅
- Neovim: 0.12.2 + LazyVim Rust extra ✅
- Cargo project: initialized, committed, pushed to GitHub ✅

---

## Session State (resume point)

Last working session ended at the boundary of Phase 3 → Phase 4.

### Current files

- `src/main.rs` — returns `Result<_, Box<dyn Error>>`; branches on trust check; calls `tailscale::up`/`down` with `?` propagation
- `src/config.rs` — loads `~/.config/tailcloak/config.toml` (XDG-aware) into `Config { trusted_ssids: HashSet<String> }`
- `src/tailscale.rs` — `up()` / `down()` shelling out via `std::process::Command`, checking `ExitStatus::success()`
- `src/wifi.rs` — **stub** returning a hardcoded `Some("...")` to be replaced in Phase 4

### Uncommitted work

- `src/main.rs` has uncommitted changes converting `let _ = tailscale::up()` → `tailscale::up()?` and changing `main` signature to return `Result`. Intended commit: `fix(main): surface tailscale errors instead of swallowing them`.

### Key decisions made (don't relitigate)

- **`trusted_ssids` is `HashSet<String>`**, not `Vec<String>`. Reason: set semantics + `HashSet::contains` accepts `&str` via `Borrow`, cleaner ergonomics than `Vec::contains(&String)`.
- **Error handling pattern**: in the current "run once" code, `main` returns `Result` and uses `?` everywhere. In Phase 5's event handlers, will switch to **log-and-continue** (`if let Err(e) = ... { eprintln!(...) }`) so one failed toggle doesn't kill the daemon.
- **No `is_tailscale_up` guard**. `tailscale up`/`down` confirmed fast on this machine; idempotent enough.
- **XDG path resolution is manual** (not via `dirs::config_dir()`) because `dirs` returns the macOS-native `~/Library/Application Support`, and we want `~/.config` (or whatever `$XDG_CONFIG_HOME` says) to match chezmoi-managed dotfiles.
- **Config file lives outside the repo.** Tracked separately in private dotfiles. Public dotfiles will eventually ship only a launchd plist template + setup script.

### Commit conventions in use

- `feat(scope): ...` — new capability
- `fix(scope): ...` — corrects wrong behavior (e.g., silent error swallowing)
- `refactor(scope): ...` — pure code change, no behavior delta
- `chore(deps): ...` — dependency bumps
- Scope mirrors module name (`config`, `tailscale`, `wifi`, `main`)

### Next session start: Phase 4

Replace the `wifi.rs` stub with a real macOS SystemConfiguration call. Open questions to resolve at the start:

1. **Which crate(s)?** Likely `core-foundation` + `system-configuration`. Confirm latest versions and that they cover `SCDynamicStoreCopyValue`. Alternative: `objc2` family for CoreWLAN's `CWWiFiClient` (higher level but heavier).
2. **Which API?** Two paths:
   - **`SCDynamicStoreCopyValue`** on key `State:/Network/Interface/en0/AirPort` — returns a CFDictionary with `SSID_STR`. Lower level, no permission prompt in some cases.
   - **`CWWiFiClient.shared().interface().ssid()`** — higher level, may require Location Services permission on recent macOS (catch: returns empty/nil without permission).
   - Decide based on what user has approved.
3. **First exercise:** write a throwaway `examples/print_ssid.rs` that just prints the current SSID, before integrating into the daemon. Lets you isolate the FFI learning.

After Phase 4 works, Phase 5 wires `SCDynamicStoreSetNotificationKeys` + `CFRunLoop` to make it event-driven.
