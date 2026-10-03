# Universal workspace release

## Accepted scope

The user selected Mihomo, sing-box and Xray. Preserve the deployed installation
and private configuration; implement and release the development copy.

- A responsive navigation rail on wide terminals and two rows on compact terminals.
- Config selection by editable path and optional native picker. Selection works
  before core installation, marks validation pending, and never starts networking.
- Portable ZIP data beside the application, plus a folder chooser and remembered
  explicit runtime selection. Switching storage does not migrate or stop processes.
- Core catalog with authors, licenses, capabilities, pinned downloads and a
  documented custom adapter format. Native profiles remain available for every core.
- A private endpoint library, selection and share-link/WireGuard imports.
- Named editable routing standards with ordered domain/process/IP rules,
  direct/proxy/block actions, browser domains and a direct default.
- Core options for DNS, listening ports, IPv6, logging and supported TUN modes.
  Profile generation creates a new private file; edits use private copies.
- Inspectable core/profile differences, validation, logs, backups and recovery.
- Secret-free public source, bilingual documentation, upstream attribution,
  expanded unit/integration/rendering tests and a GPG-signed publication.

## Compatibility and boundaries

Retain legacy Mihomo commands/settings as compatibility entry points. A separate
schema-1 workspace adds per-core profiles and preferences. Secrets remain inside
protected private runtime files. Structured UI requests go through stdin rather
than command-line arguments. Only PID/path/creation-time owned processes may stop.
Downloads and all native validators are exercised in temporary roots. Core starts
in integration use random loopback ports, TUN disabled and no system proxy changes.

Generated profiles reflect an explicit selected endpoint, standard and core
options. Unsupported combinations are rejected with a clear explanation; native
config files provide advanced vendor features. Running profiles stay unchanged
until the user explicitly stops/starts. DNS controls configure clients, not Windows
adapter DNS. Native ARM64 execution is not claimed without hardware verification.

## Review findings to address

The initial dashboard lacked a per-core model, profile editing/validation feedback,
structured endpoint/rule libraries, core-specific DNS controls, install location
selection, visible attribution and a real UI/process integration test. Windows
separator comparison and publication verification also need regression coverage.
The tagged v1.0.0 preflight is not a published stable release. The expanded release
is v1.1.0; existing tags are retained.

Private JSON/YAML and stdin requests use explicit UTF-8 on Windows PowerShell 5.1.
Non-ASCII profile paths and endpoint/standard names must survive save/reload.
Test the real structured backend request pipe and real local proxy forwarding.
