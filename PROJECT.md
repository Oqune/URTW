# URTW project map

Public Windows routing dashboard, runtime contract v1, release line v1.1.x.
This development repository is independent from any existing deployed installation.

| Area | Entry point |
|---|---|
| Product and setup | README.md / README.ru.md |
| Current specification | docs/specs/2026-10-03-universal-workspace.md |
| Architecture | docs/ARCHITECTURE.md |
| Version/release policy | docs/policies/policies.md |
| Security | SECURITY.md |
| UI and asynchronous events | src/ui.rs / src/app.rs / src/main.rs |
| Status and scoped execution | src/system.rs |
| Routing/component backend | routing.ps1 |
| Pinned verified downloads | components.lock.json |
| CI / release | .github/workflows/build.yml / scripts/ |
| Release acceptance evidence | docs/audit/compatibility-v1.1.0.md |

No private profiles or installed component binaries belong in source control.
