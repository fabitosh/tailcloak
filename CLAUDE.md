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

### Phase 0: Dev Environment

- [x] rustup + stable toolchain (1.96.0, aarch64-apple-darwin)
- [x] LazyVim: enable the built-in Rust extra (`~/.config/nvim/lua/plugins/`) — ships `rustaceanvim` (LSP, inlay hints, hover docs), `nvim-dap` (debugger), and rustfmt via `conform.nvim`
- [ ] Key cargo commands to internalize: `cargo check` (fast type-check), `cargo clippy` (linter), `cargo fmt`, `cargo doc --open`
- **Learning goal:** understand Cargo as both build system and package manager; get the edit→check loop fluid in LazyVim before writing real logic

### Phase 1: Project Bootstrap

- `cargo new tailcloak --bin` — read and understand the generated structure
- Write a minimal but correct `Cargo.toml` (name, version, edition = "2021", authors)
- **Concepts:** crate vs package, `Cargo.lock` (commit it for binaries), editions, `src/main.rs` entry point
- **Learning goal:** Cargo project anatomy; how `use`, `mod`, and file layout relate

### Phase 2: Configuration System (Private SSIDs)

- Strategy: `~/.config/tailcloak/config.toml` — never inside the repo
- Add dependencies: `serde` (with `derive` feature), `toml`, `dirs` (XDG paths)
- Write a `Config` struct with `trusted_ssids: Vec<String>`; derive `Deserialize`
- Parse at startup with a clear error if the file is missing or malformed
- **Concepts:** structs, `impl` blocks, `derive` macros, `Result<T, E>`, the `?` operator, file I/O with `std::fs`
- **Learning goal:** Rust's error handling model; how `serde` + `toml` turn a file into a typed struct

### Phase 3: Core Logic + Tailscale Subprocess

- Create a `tailscale` module (`src/tailscale.rs`): `pub fn up()` and `pub fn down()` using `std::process::Command`
- Stub `get_current_ssid() -> Option<String>` returning a hardcoded value for now
- Wire the decision: `if config.trusted_ssids.contains(&ssid) { tailscale::down() } else { tailscale::up() }`
- **Concepts:** modules and visibility (`pub`, `mod`), `std::process::Command`, `String` vs `&str`, `Option<T>`, pattern matching (`match`, `if let`)
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
- LazyVim Rust extra: pending setup
- Cargo project: not yet created
