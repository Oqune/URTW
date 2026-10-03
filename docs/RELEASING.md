# Maintainer release workflow

CI automatically tests changes and builds portable Windows x64/ARM64 artifacts.
A verified GPG-signed tag creates a **draft** GitHub release. Signing remains
local, so the existing private key is not exported into Actions secrets.

1. Update VERSION, Cargo.toml, CHANGELOG.md and `docs/releases/vX.Y.Z.md`.
2. Run all gates in CONTRIBUTING.md; inspect the intended public source.
3. Commit using Conventional Commits; create a signed tag:

```powershell
git tag -s v1.0.0 -m "URT v1.0.0"
git push origin master
git push origin v1.0.0
```

4. Wait for the Build and Release workflow to pass.
5. Run the local publication gate:

```powershell
powershell -File scripts/publish-release.ps1 -Version 1.0.0
```

If GPG is not on PATH, pass its installed executable with `-Gpg`, for example
`-Gpg "C:\Program Files\Git\usr\bin\gpg.exe"`. This selects the existing local keyring.

The gate verifies the signed tag and successful CI commit, downloads the draft
archives and SHA256SUMS, recomputes every hash, runs the packaged secret scan,
signs the exact manifest with GPG, verifies that signature, attaches it and
publishes. Signing may use the normal local GPG PIN entry; do not pass a private
key or passphrase in command-line arguments or commit it into configuration.

A workflow rerun may refresh a draft, but cannot overwrite a published release.
Do not clobber released artifacts or move tags. A correction requires a new patch.
For key rotation, publish a signed transition statement and update the fingerprint
and CI verification anchor through a reviewed source change.
