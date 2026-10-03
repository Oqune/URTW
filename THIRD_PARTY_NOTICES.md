# Third-party components

URT is MIT-licensed. Its portable ZIPs contain no upstream network executables.
Downloaded components retain their own licenses and upstream ownership:

| Component | Source | License / distribution |
|---|---|---|
| Mihomo | https://github.com/MetaCubeX/mihomo | GPL-3.0; see upstream LICENSE |
| Zapret package | https://github.com/Flowseal/zapret-discord-youtube | See package/upstream license; WinDivert has its own license |
| TG WS Proxy | https://github.com/Flowseal/tg-ws-proxy | MIT |
| Wintun | https://www.wintun.net/ | Official prebuilt signed DLL distribution license from its ZIP |
| Rust crates | Cargo.lock / crates.io | Licenses declared by each crate |

The installer retrieves upstream packages without modifying their strategies or
configurations. The Wintun binary license is retained beside its installed DLL.
