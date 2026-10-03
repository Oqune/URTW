# Release v1.0.0 acceptance record

## Scope and data boundary

This public repository is independent of the existing Windows deployment.
Runtime configuration, WireGuard credentials, Telegram secrets, logs, installed
binaries and caches are excluded. Packaging explicitly selects public assets.
The source and ZIP credential scanner reports file/rule names without values.

## Checks

- Rust format and clippy with warnings denied.
- Eight Rust tests: responsive tab rendering, modal/operation states, selection
  scrolling and Unicode width; diagnostic completion with errors; loopback HTTP
  response behavior and numeric progress boundaries.
- 39 isolated PowerShell checks: private ACLs, settings serialization, runtime
  path boundaries, proxy ownership comparison, strict synthetic WireGuard import,
  SHA-256 verified downloads, actual Mihomo config validation, archive traversal
  rejection and process ownership.
- Isolated Mihomo starts on a random loopback port with TUN/DNS disabled, stops
  only its recorded process, releases its listener and leaves Windows Proxy unchanged.
- UI snapshots at supported terminal sizes and source/portable ZIP scanning.
- Release metadata, signed tag, archive hashes and detached checksum signature
  are enforced by CI and the local publication gate.

## Practical limits

These checks validate control flow and isolated local operation. They do not
claim end-to-end access through a user's VPN credentials or bypass of any
specific block. Global TUN/network switching is not part of development checks.
The real Telegram client path and Zapret service installation are delegated to
upstream tools and require the user's explicit product action. ARM64 is built
and packaged in CI; native ARM64 hardware execution remains unverified.

GPG signs the Git tag and checksum manifest. The Windows executables do not
carry an Authenticode signature. Upstream component archives use pinned SHA-256
digests; their licensing and signing are independent from URT.
