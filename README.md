# tailcloak

A small native macOS background daemon that turns Tailscale **on** for untrusted
networks and **off** for trusted ones — automatically, as you move between them.

Networks are identified by their **default-gateway MAC address**, not their Wi-Fi
SSID. The gateway is harder to spoof than an SSID, stays stable across SSID
rebrands, and reading it needs no Location Services permission. A network counts
as trusted if its gateway MAC is in your trusted list.

> Built as a Rust learning project. macOS only.

## How it works

The daemon watches macOS SystemConfiguration for network changes (event-driven,
no polling). On every change it reconciles:

```
current gateway MAC ∈ trusted set  ->  tailscale down
otherwise (incl. no network)       ->  tailscale up
```

Reconciliation is **level-triggered and self-healing**: the desired Tailscale
state is re-asserted on every network change, so a manual `tailscale down` on an
untrusted network is corrected back up. The paranoid default — unknown or absent
network — is always `up`.

## Requirements

- macOS
- Rust (stable) to build
- [Tailscale](https://tailscale.com/) installed, with the `tailscale` CLI on a
  standard path (`/usr/local/bin` or Homebrew)

## Install

```sh
cargo install --path .     # builds and installs the binary to ~/.cargo/bin
tailcloak install          # writes a LaunchAgent and starts it at login
```

`tailcloak install` generates a launchd LaunchAgent at
`~/Library/LaunchAgents/dev.fmeier.tailcloak.plist` pointing at the installed
binary, then loads it. It runs at login and restarts on crash. Logs go to
`~/Library/Logs/tailcloak.log`.

Re-running `tailcloak install` after an upgrade is safe — it reloads the agent.

## Configure trusted networks

While connected to a network you trust:

```sh
tailcloak trust-current      # add this network's gateway MAC to the trusted set
tailcloak distrust-current   # remove it
tailcloak show-trusted       # list trusted gateway MACs
```

Trusted MACs are stored in `~/.config/tailcloak/config.toml` (outside this repo):

```toml
trusted_gateway_macs = ["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"]
```

Edits apply on the next network change. With no config file, every network is
treated as untrusted (Tailscale stays up).

## Commands

| Command | Description |
| --- | --- |
| _(none)_ | Run the daemon in the foreground (what the LaunchAgent runs) |
| `run-once` | Reconcile once and exit |
| `trust-current` | Trust the current network's gateway |
| `distrust-current` | Untrust the current network's gateway |
| `show-trusted` | List trusted gateway MACs |
| `install` | Install and start the LaunchAgent |
| `uninstall` | Stop and remove the LaunchAgent |

## Manage the service

```sh
tail -f ~/Library/Logs/tailcloak.log                          # watch activity
launchctl print gui/$(id -u)/dev.fmeier.tailcloak             # inspect state
launchctl bootout gui/$(id -u)/dev.fmeier.tailcloak           # stop until next login
tailcloak uninstall                                           # remove entirely
```

To pause it temporarily, `bootout` stops it until your next login. `tailcloak
uninstall` removes it for good.

## Uninstall

```sh
tailcloak uninstall
cargo uninstall tailcloak   # if installed via cargo install
```
