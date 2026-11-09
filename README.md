# tailcloak

A small native macOS background daemon that enforces your Tailscale setup per
network: e.g. Tailscale up with an exit node on untrusted networks, hands off on
trusted ones. Automatically, as you move between them.

Networks are identified by their default-gateway MAC address. A network counts
as trusted if its gateway MAC is in your trusted list.

> Built as a Rust learning project. macOS only.

## Why

On a café, hotel, or conference Wi-Fi I want Tailscale up and routed through my
exit node, so my traffic is encrypted past the local network and leaves from
somewhere I control. On my trusted networks I don't need that, and I decide
myself whether Tailscale is on.

Doing that by hand means flipping Tailscale and its exit node on every network
change, and a forgotten `tailscale up` on hostile Wi-Fi is exactly what I want
to avoid. tailcloak makes it automatic.

## How it works

The daemon watches macOS SystemConfiguration for network changes. On every change it reconciles:

```
current gateway MAC ∈ trusted set  ->  apply the [trusted] policy
otherwise (incl. no network)       ->  apply the [untrusted] policy
```

Enforced settings are re-applied on every network change, so a manual change
to them only lasts until the next one (use `pause` for a temporary override).

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

Edits to the config apply on the next network change or directly with
`tailcloak run-once`.

## Configuration

Everything lives in `~/.config/tailcloak/config.toml` (outside this repo):

```toml
trusted_gateway_macs = ["aa:bb:cc:dd:ee:ff", "00:11:22:33:44:55"]

[trusted]
tailscale = "unmanaged"   # leave up/down to me at home
exit_node = "off"         # but never keep an exit node on

[untrusted]
tailscale = "up"
exit_node = "auto:any"    # or a node name / Tailscale IP
```

Each policy has two settings:

| Key         | Values                              | Meaning                                                                 |
| ----------- | ----------------------------------- | ----------------------------------------------------------------------- |
| `tailscale` | `"up"`, `"down"`, `"unmanaged"`     | Enforce the connection state, or never touch it                         |
| `exit_node` | `"off"`, `"unmanaged"`, `<node>`    | Clear the exit node, leave it alone, or enforce this node (IP, name, `auto:any`) |

- A section must set both keys. `tailscale = "down"` with a named exit node is rejected.
- Missing sections keep the original behavior: `[trusted]` = `down` / `unmanaged`,
  `[untrusted]` = `up` / `unmanaged`. A config with only `trusted_gateway_macs`
  works unchanged.
- Unknown keys are an error, so a typo can't silently disable a setting. If the
  config can't be read, tailcloak applies the untrusted default (Tailscale up)
  and logs the error.
- An enforced exit node must be offered in your tailnet (`tailscale exit-node list`).
  If it is unreachable, Tailscale blocks internet traffic rather than leaking it;
  `tailcloak pause <min>` gets you out, e.g. to log in to a captive portal.

## Commands

| Command             | Description                                                  |
| ------------------- | ------------------------------------------------------------ |
| _(none)_            | Run the daemon in the foreground (what the LaunchAgent runs) |
| `run-once`          | Reconcile once and exit                                      |
| `trust-current`     | Trust the current network's gateway                          |
| `distrust-current`  | Untrust the current network's gateway                        |
| `show-trusted`      | List trusted gateway MACs                                    |
| `pause <minutes>`   | Take Tailscale down and suspend enforcing for N minutes      |
| `resume`            | Resume enforcing now                                         |
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
