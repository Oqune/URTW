# Compatibility and release audit — v1.1.0

The public source is independent of a personal installation. Updating a deployed
legacy installation must preserve its native config, credentials, TG data, Zapret
strategies, component versions and scheduled tasks. Do not generate replacement
rules as a migration step. Keep root legacy scripts; deploy app assets separately.

| Existing capability | v1.1.0 treatment |
|---|---|
| Native Mihomo selective rules / WG / DNS | Original native config selected intact; endpoint copy imported privately for future generation |
| Browser and per-application split | Original native semantics preserved; editable standards/PAC added for future profiles |
| TUN / proxy mode | Original profile retained; explicit per-core options; no migration network switches |
| Telegram WS / secret / upstream tray options | Existing binary/config path aliases; no replacement or restart |
| Zapret strategies and domain lists | Original service.bat and package retained |
| Config imports and multiple profiles | Per-core library, pending validation, native picker/path entry, editable copies |
| Diagnostics / progress | Actual local inbound or system path, HTTP responses separated from transport failure |
| Original Boot / autostart task | Original scripts/task unchanged; explicit separate URTW core autostart available |
| StartAll/StopAll, reload, list update, IDE sync, repair/cleanup | Original root commands retained in the deployed legacy script; new UI uses scoped individual controls |
| Updates and rollback | Public app files independent of private configs; private backup before deployment; immutable signed releases |

There is no promise that every legacy automation button has a new UI equivalent.
Legacy commands remain available in the original installation, including behaviors
that may change metrics, IDE settings or global variables. They are not invoked
by deployment or normal URTW startup. Existing private native profiles keep vendor
features beyond the generator's supported subset.

Release gates: Rust formatting/clippy/unit/render tests; PowerShell parsers and
private-state tests; real pinned binary validators for direct/WG/share-link/fallback
profiles; each core runs TUN off on random loopback ports and forwards local HTTP;
real Rust snapshots verify managed ownership; Windows proxy is compared before/after.
TG/Zapret packages install without launching. Source and archives undergo private
input/credential scans. Both CI architectures are verified, GPG validates tag and
signed exact archive checksums. Native ARM64 runtime and global TUN switches are
not exercised on the user's live PC.
