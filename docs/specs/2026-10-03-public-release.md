# Public release contract

## Scope

Extract URTW development from the existing Windows installation. Keep the active
installation and every private configuration unchanged. Publish source and
portable x64/ARM64 releases with signed tags and detached GPG checksum signatures.

## Runtime contract v1

- Assets live beside `URTW.exe`; mutable data defaults to `%LOCALAPPDATA%\URTW`.
- `--root` explicitly selects a different runtime. No machine-specific fallback.
- Startup reads status only. Installing, selecting, starting and stopping are
  distinct operations. Selecting a profile never restarts a service.
- Only recorded processes with the matching executable path can be stopped.
- Record the running Mihomo port separately from the next selected profile.
  Diagnostics use the running port; Windows Proxy activation rejects a selection
  that differs from the running core. Re-select changed mixed-port values before start.
- Normalize Windows paths before ownership comparison. Isolated integration
  must confirm that the built Rust dashboard recognizes the core as managed.
- Native file picker supports Mihomo YAML and single-peer WireGuard profiles.
- Profiles and Telegram secrets are private runtime data, never release assets.
- Zapret management opens upstream `service.bat`. No preset picker or automatic
  replacement of Windows services, routes, adapter metrics or IDE settings.
- Telegram uses the official portable tray application, offers start/stop,
  configuration, connection link and upstream instructions.
- Downloads use pinned releases and SHA-256 checks before extraction/execution.
- Diagnostic completion includes unsuccessful results; HTTP responses and
  network errors are shown separately. HTTPS certificate validation stays on.

## UI acceptance

Six tabs, consistent single-cell padding, explicit empty/error states, native
profile picker, real asynchronous operation status. No fabricated topology,
health, versions or bandwidth. Small windows display a resize hint; supported
windows from 60x18 through 200x60 render without overflow or panic. Diagnostics
show both completion and HTTP results. Selected rows stay in the viewport.

## Release acceptance

Format, clippy, Rust tests, PowerShell parser and isolated engine tests; secret
scan of source and portable ZIPs; consistent VERSION/Cargo/tag; bilingual docs;
architecture table and verification guide. CI builds all supported architectures
and creates draft releases. Publication requires verified GPG-signed checksums.
Isolated integration installs each x64 upstream package without launching
Telegram or Zapret, checks Telegram config preservation and refuses Zapret
package replacement that could overwrite customized upstream files.
