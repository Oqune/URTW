# Security

Report vulnerabilities privately using
[GitHub security advisories](https://github.com/Oqune/URT/security/advisories/new).
Do not place credentials in public issues. The supported release line is v1.0.x.

## Boundaries

- Dashboard startup reads status; state-changing actions are explicit.
- Control of Mihomo and Telegram requires a matching PID, executable path and
  process creation time recorded inside the selected runtime.
- External processes, services and proxy settings are not owned by URT.
- New runtime directories and private files receive protected Windows ACLs for
  the current user, SYSTEM and Administrators. These ACLs do not protect data
  against the same user, an administrator, malware running as that user or backups.
- VPN profiles and Telegram secrets are plaintext local component inputs, not
  encrypted password storage. Component logs can contain sensitive values.
- HTTPS diagnostics validate certificates. The installer validates HTTPS URLs,
  pinned SHA-256 digests and archive paths before using downloads.
- A digest pins bytes; it does not replace trust in upstream maintainers. URT
  ZIPs have GPG-signed checksums. GPG signatures are distinct from Windows
  Authenticode and do not remove SmartScreen prompts.
- A selected arbitrary YAML can itself request TUN, providers, remote downloads
  or other Mihomo functionality. Only import configurations you trust.

Private signing keys never enter this repository, release ZIPs or CI. CI creates
draft releases. Publication verifies signed tags and signs the artifact checksum
manifest using the maintainer's local GPG key.
