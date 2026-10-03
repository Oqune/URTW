# URTW engineering contract

- Read `PROJECT.md`, `docs/policies/policies.md` and the relevant specification.
- Never change an existing deployed installation or private configuration as a
  side effect of development. Work in this repository and isolated temporary roots.
- Never commit real profiles, runtime state, credentials, logs or third-party binaries.
- Changes to routing require explicit user actions in the product. Keep startup read-only.
- Use Conventional Commits and signed release tags `vX.Y.Z`.
- Before release: `cargo fmt --check`, `cargo clippy --locked -- -D warnings`,
  `cargo test --locked`, `scripts/test-engine.ps1`, `scripts/check-release.ps1`.
- Document new behavior before implementing it; update architecture and user docs.
- Do not claim measured network properties without a real measurement.
