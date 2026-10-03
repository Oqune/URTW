# Contributing

Read `AGENTS.md`, `PROJECT.md` and the current specification before changing behavior.
Use a dedicated runtime directory outside your active network installation.
Do not test with personal VPN profiles or enable a system TUN in the test suite.

```powershell
cargo fmt --check
cargo clippy --locked -- -D warnings
cargo test --locked
powershell -NoProfile -File scripts/test-engine.ps1 -Integration
powershell -NoProfile -File scripts/check-release.ps1
```

The engine integration test downloads a pinned core into a temporary private
directory and starts only a TUN-disabled proxy on a random loopback port.
It verifies that Windows proxy settings stay unchanged and cleans up its own
recorded process. UI tests exercise six tabs, Unicode, scroll boundaries,
progress and window sizes from 1×1 to 200×60 without changing networking.

Use English Conventional Commits and short branches. The default `master`
branch must pass CI. Explain user-visible behavior and security boundaries in
a specification; update both READMEs when controls or deployment change.

Do not attach real keys, endpoint profiles, Telegram secrets or raw logs to
issues/PRs. Pin upstream version changes with their official SHA-256 digests.
Third-party binaries remain runtime downloads, rather than Git history.
