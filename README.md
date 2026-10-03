<div align="center">

# URT

**Universal Routing Toolset — a Windows terminal dashboard for selective routing**

[English](README.md) | [Русский](README.ru.md)

[![CI](https://github.com/Oqune/URT/actions/workflows/build.yml/badge.svg)](https://github.com/Oqune/URT/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/Oqune/URT)](https://github.com/Oqune/URT/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

</div>

URT manages **Mihomo**, the upstream **Zapret service manager**, and **Telegram
WS Proxy** from one Rust/Ratatui dashboard. It reads actual processes and local
listeners, selects private configurations, and reports asynchronous operations.
Opening the dashboard does not start services or change Windows networking.

![Diagnostics UI with sample results](docs/images/diagnostics.png)

*Read-only sample preview; results in normal use are measured on request.*

## Download and first run

1. Download the portable ZIP for your architecture from [Releases](https://github.com/Oqune/URT/releases).
2. [Verify its GPG signature and SHA-256](docs/VERIFICATION.md), then extract it.
3. Run `URT.exe`. On **Components**, select a component and press **I** to install
   its pinned official release. Downloads are checked against `components.lock.json`.
4. Press **F** and select a Mihomo `.yaml`/`.yml` or single-peer WireGuard `.conf`.
   Selecting validates the profile; **S** starts Mihomo separately.

Windows 10/11 x64 and Windows ARM64 are supported. Zapret/WinDivert integration
is **x64 only**. Administrator access is required when a selected profile enables
TUN, and for upstream Zapret service management. Ordinary dashboard inspection,
proxy-only Mihomo and Telegram installation do not require elevation.

| Architecture | Dashboard | Mihomo | TG WS Proxy | Zapret |
|---|---|---|---|---|
| Windows x64 | Yes | Yes | Yes | Yes |
| Windows ARM64 | Yes | Yes | Yes | Not supported |

The release contains URT and public examples, without VPN credentials or
third-party executables. Component downloads need an already working internet
connection. If that connection currently relies on another VPN, install and
validate your new profile before manually switching providers.

## Dashboard

| Tab | What it does | Keys |
|---|---|---|
| Overview | Real process/listener state and Windows proxy ownership | F: profile; S/X: Mihomo; P: Windows proxy |
| Profiles | Native file picker, saved profile list, validation, separate activation | F: browse; Enter: select; S/X: start/stop |
| Diagnostics | HTTP status and elapsed request time; completion counts failures too | D: run; M: Mihomo inbound/system path; arrows: scroll |
| Telegram | Portable tray app, local port, private settings and connection link | I: install; S/X: start/stop; L: copy link; G: Telegram; E: port; O: settings |
| Zapret | Opens the official `service.bat` in its own console | Enter: manager; I: install |
| Components | Installed versions recorded by URT and pinned upstream versions | Arrows: select; I/Enter: install |

Use **1–6**, **F1–F6**, or **Tab/Shift+Tab** to change tabs. **Q/Esc** closes the
dashboard; managed services keep running. Supported layout starts at **60×18**
cells and adapts to wider windows. Unmanaged processes are explicitly labelled
**External**; URT never stops them by name.

## Profiles and two routing layers

A selected YAML stays in its original location and is never rewritten by
selection. URT requires an explicit top-level `mixed-port`. Runtime cache and
providers use the private `mihomo-data` directory; external YAML providers should
use absolute paths or be placed there. Validation does not prove VPN reachability.

WireGuard import accepts one peer, optional preshared key, IPv4 plus optional
IPv6 address, bracketed IPv6 endpoints, DNS IPs, MTU and keepalive. It rejects
executable hooks and unknown directives. Import creates a new private YAML,
preserves the original `.conf`, and defaults to **TUN disabled / rule mode /
catch-all DIRECT**. Importing is not applying a new profile to a running core.

Generated profiles have two layers:

1. Mihomo routes selected executable names and domain suffixes through the imported
   WireGuard peer; local networks and remaining traffic use DIRECT.
2. A separate `browser-routing.pac` chooses browser sites from
   `browser-vpn-domains.txt`; other sites use the system path. The PAC includes a
   direct fallback if the local proxy is unavailable.

To customize defaults, copy `config/rules/` to your private runtime `rules/` and
edit the lists, then import the WireGuard profile again. Already generated YAML
is not automatically changed. For full system capture, edit the private generated
YAML to set `tun.enable: true`, then start it from an Administrator terminal.
Explicit app rules take precedence over domain rules; DIRECT domains precede
VPN domain lists. Review these priorities before changing the lists.

[Browser PAC setup](docs/BROWSER.md) explains per-browser configuration.
HTTP diagnostics through the Mihomo inbound exercise its rules. The **System
path** disables the HTTP proxy client setting but can still traverse an active
system TUN; HTTP 403/429 is not by itself evidence of a routing failure.

## Telegram WS Proxy

On Telegram, press **I**, then **S**. URT generates a private random secret,
binds `127.0.0.1`, and starts the [official portable tray app](https://github.com/Flowseal/tg-ws-proxy).
Press **L** to copy its `tg://proxy` link or **G** to open Telegram's connection
prompt. Telegram uses MTProto locally and WebSocket upstream.

The upstream app allows one instance per Windows user. An existing external
instance is managed from its tray. **E** changes the port while the managed
instance is stopped; **O** opens the private configuration. Advanced DC,
Cloudflare and WebSocket settings remain available in the upstream tray.
URT supports ordinary 32-hex secrets for link generation; advanced secret
formats use the upstream link. See [the Telegram guide](docs/TELEGRAM.md).

## Data and safety

Assets stay beside the executable. Mutable state defaults to `%LOCALAPPDATA%\URT`.

```text
Release directory                Private runtime
URT.exe                          settings.json
routing.ps1 / setup.ps1          profiles/ (generated VPN YAML)
components.lock.json             browser-routing.pac
config/examples/                 rules/ (optional overrides)
config/rules/                    tools/ (verified component downloads)
docs/                            mihomo-data/ and logs/
```

Override the runtime deliberately with `URT.exe --root "D:\URT-data"` or `URT_HOME`.
URT uses protected Windows ACLs for new runtime directories and private files.
It does not change adapter metrics, IDE settings, shell proxy variables, firewall
rules or unrelated Windows services. Windows proxy changes are explicit and
restore their saved settings only while ownership still matches. Stopping managed
Mihomo restores its owned Windows proxy before stopping the core.

Logs and configuration files may contain private data; they are not release
assets and must not be attached to issues without review. See [SECURITY.md](SECURITY.md).

## Build and release

Requires Rust **1.88+**, the MSVC toolchain and Windows PowerShell 5.1.

```powershell
cargo fmt --check
cargo clippy --locked -- -D warnings
cargo test --locked
powershell -File scripts/test-engine.ps1 -Integration
powershell -File scripts/check-release.ps1
cargo run -- --root "D:\URT-development-data"
```

[Version policy](docs/policies/policies.md) follows Impulse's contract-major
SemVer and Conventional Commits: **`v<ContractMajor>.<FeatureMinor>.<Patch>`**.
Contract v1 is URT's runtime/CLI schema, independent of Impulse's network protocol.
CI tests changes and builds portable x64/ARM64 archives. Signed tags create
**draft** releases; [local GPG verification/signing](docs/RELEASING.md) publishes
them after their checksums have been signed. The private signing key stays local.

[Architecture](docs/ARCHITECTURE.md) · [Contributing](CONTRIBUTING.md) ·
[Release verification](docs/VERIFICATION.md) · [Changelog](CHANGELOG.md)
