# tailcloak

A small native macOS background daemon that turns Tailscale on for untrusted
networks and off for trusted ones. Automatically, as you move between them.

Networks are identified by their default-gateway MAC address. A network counts
as trusted if its gateway MAC is in your trusted list.

> Built as a Rust learning project. macOS only.

## Why

On a café, hotel, or conference Wi-Fi I want Tailscale up and routed through my
exit node, so my traffic is encrypted past the local network and leaves from
somewhere I control. On my trusted networks I want Tailscale down: I reach LAN
services directly at full speed.

Doing that by hand means flipping tailscale on/off on every network change, and a forgotten
`tailscale up` on hostile Wi-Fi is exactly what I want to avoid. tailcloak
makes it automatic.

tailcloak only toggles tailscale up or down. The exit node or route configuration is set once and will be kept.

## How it works

The daemon watches macOS SystemConfiguration for network changes. On every change it reconciles:

```
current gateway MAC ∈ trusted set  ->  tailscale down
otherwise (incl. no network)       ->  tailscale up
```

> [!NOTE]
> Gateway MAC rather than SSID: it's harder to spoof, survives network renames,
> and needs no Location Services permission (macOS hides SSIDs from the terminal
> without it).

## Requirements

- macOS
- [Tailscale](https://tailscale.com/) installed

## Install

Install the binary with Homebrew:

```sh
brew install fabitosh/tap/tailcloak
```

or with cargo:

```sh
cargo install tailcloak
```

Then start the background agent:

```sh
tailcloak service install
```

The two steps are deliberately separate: the package manager owns the binary,
while `tailcloak service` owns the background agent. You can use tailcloak
without the service.

Re-running `tailcloak service install` after an upgrade is safe — it reloads the
agent.

## Regular Usage

By default all networks are untrusted. If you have e.g. an exit-node running on your tailscale and need to authenticate within the new network, you can use

```sh
tailcloak pause 5 # pauses tailcloak and takes tailscale down
```

While connected to a network you trust:

```sh
tailcloak trust-current      # add this network's gateway MAC to the trusted set
```

Trusted MACs are stored in `~/.config/tailcloak/config.toml` (outside this repo):

```toml
trusted_gateway_macs = ["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"]
```

Edits apply on the next network change or directly with `tailcloak run-once`.

## Commands

| Command             | Description                                                  |
| ------------------- | ------------------------------------------------------------ |
| _(none)_            | Run the daemon in the foreground (what the LaunchAgent runs) |
| `run-once`          | Reconcile once and exit                                      |
| `trust-current`     | Trust the current network's gateway                          |
| `distrust-current`  | Untrust the current network's gateway                        |
| `show-trusted`      | List trusted gateway MACs                                    |
| `pause <minutes>`   | Suspend toggling for N minutes                               |
| `resume`            | Resume toggling now                                          |
| `service install`   | Install and start the LaunchAgent                            |
| `service uninstall` | Stop and remove the LaunchAgent                              |

## Manage the service

```sh
tail -f ~/Library/Logs/tailcloak.log                 # watch activity
launchctl print gui/$(id -u)/dev.fmeier.tailcloak    # inspect the launchd job
```

## Uninstall

First stop and remove the background agent:

```sh
tailcloak service uninstall
```

Then remove the binary the same way you installed it:

```sh
brew uninstall tailcloak       # if installed via Homebrew
cargo uninstall tailcloak      # if installed via cargo
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
