# Engineering and version policy

## Commits and branches

English Conventional Commits, no emoji, one logical change per commit. Default
branch `master` stays releasable. Short feature/fix branches merge after format,
clippy, Rust tests, PowerShell tests and source/release secret checks.

## Contract-Locked SemVer

URT adopts Impulse's contract-major version discipline:
`v<ContractMajor>.<FeatureMinor>.<Patch>`.

- **ContractMajor** increments when the public CLI, persisted settings schema or
  managed component integration contract becomes incompatible. Current contract: 1.
- **FeatureMinor** increments for compatible functionality, new screens or controls.
- **Patch** increments for fixes, security hardening, performance and UI refinement.
- Upstream Mihomo/Zapret/Telegram versions are independent pinned component versions.
- Impulse's E2EE protocol major is not inherited by this independent product.

`VERSION`, `Cargo.toml`, signed Git tag and release title must agree. Tags are
strictly `vX.Y.Z`; titles are `URT vX.Y.Z`. Notes are English/Russian, contain no
emoji, include architecture tables with direct artifact links and describe
meaningful limits. Published releases and tags are immutable; fixes get new versions.

## Release gates

1. Specification and documentation updated.
2. Format, clippy, tests and isolated proxy-only integration pass.
3. Source and packaged assets pass the credential/private-path scanner.
4. Signed tag verifies against `SIGNING_KEY_FINGERPRINT`.
5. CI builds both Windows architectures and creates a draft with SHA256SUMS.
6. Local GPG signs the exact built artifact manifest; local verification passes.
7. Publication only after signed checksums are attached. No private key in CI.

## Private data

Runtime roots, private profiles, generated YAML, TG secrets, logs, caches,
downloads, backups and signing private keys never enter Git or release archives.
Source and packaging use allowlists; new private inputs use Windows ACLs.
