mod app;
mod model;
mod network;
mod system;
mod theme;
mod ui;
mod workspace;

use anyhow::{Context, Result, bail};
use app::App;
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEventKind, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use model::{Action, Component, Tab};
use ratatui::{
    Terminal,
    backend::{CrosstermBackend, TestBackend},
    style::Color,
};
use std::{io::stdout, path::PathBuf, time::Duration};
use system::Paths;

#[derive(Default)]
struct Options {
    root: Option<PathBuf>,
    assets: Option<PathBuf>,
    snapshot: Option<PathBuf>,
    width: u16,
    height: u16,
    tab: usize,
    preview: bool,
}
fn options() -> Result<Option<Options>> {
    let mut opts = Options {
        width: 120,
        height: 30,
        ..Options::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "URTW {}\n\nWindows routing dashboard. Startup only reads local status.\n\nURTW [--root ABSOLUTE_PATH] [--assets ABSOLUTE_PATH]\nURTW --snapshot FILE.json [--width 120 --height 30 --tab 1] [--preview]\n\nRuntime: portable data folder, --root, URTW_HOME, or %LOCALAPPDATA%\\URTW. Assets: beside URTW.exe.\nF: enter config path; N: native picker; S/X: start/stop; 1..9/0 or Tab: navigation.\n--preview uses sample data and is available only with --snapshot.",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(None);
            }
            "--version" | "-V" => {
                println!("URTW {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--root" => opts.root = Some(args.next().context("--root needs a path")?.into()),
            "--assets" => opts.assets = Some(args.next().context("--assets needs a path")?.into()),
            "--snapshot" => {
                opts.snapshot = Some(
                    args.next()
                        .context("--snapshot needs an output file")?
                        .into(),
                )
            }
            "--width" => opts.width = args.next().context("--width needs a number")?.parse()?,
            "--height" => opts.height = args.next().context("--height needs a number")?.parse()?,
            "--tab" => {
                opts.tab = args
                    .next()
                    .context("--tab needs a number from 1 to 10")?
                    .parse::<usize>()?
                    .checked_sub(1)
                    .context("Tab numbers begin at 1")?
            }
            "--preview" => opts.preview = true,
            _ => bail!("Unknown option: {arg}; use --help"),
        }
    }
    if !(1..=500).contains(&opts.width)
        || !(1..=200).contains(&opts.height)
        || opts.tab >= Tab::ALL.len()
    {
        bail!("Invalid snapshot dimensions or tab number");
    }
    if opts.preview && opts.snapshot.is_none() {
        bail!("--preview is restricted to read-only snapshots");
    }
    Ok(Some(opts))
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        );
    }
}

fn color(color: Color, default: &str) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Black => "#000000".into(),
        Color::White => "#ffffff".into(),
        Color::Red => "#ef4444".into(),
        Color::Green => "#4ade80".into(),
        Color::Yellow => "#fbbf24".into(),
        Color::Blue => "#6366f1".into(),
        Color::Cyan => "#38bdf8".into(),
        Color::Magenta => "#c084fc".into(),
        _ => default.into(),
    }
}
fn snapshot(mut app: App, opts: &Options) -> Result<()> {
    app.initialized = true;
    app.tab = Tab::ALL[opts.tab];
    app.preview = opts.preview;
    if opts.preview {
        use model::*;
        let catalog: serde_json::Value =
            serde_json::from_slice(&std::fs::read(app.paths.assets.join("config/cores.json"))?)?;
        app.snapshot.cores = serde_json::from_value(catalog["cores"].clone())?;
        app.snapshot.workspace = serde_json::from_slice(&std::fs::read(
            app.paths.assets.join("config/workspace-defaults.json"),
        )?)?;
        app.snapshot.workspace.profiles = (1..=8)
            .map(|i| crate::workspace::Profile {
                id: format!("sample-{i}"),
                name: format!("Profile {i}"),
                core: "mihomo".into(),
                path: format!(r"C:\URTW-preview\profiles\profile-{i}.yaml").into(),
                port: 17890,
                validation: "valid".into(),
                generated: true,
            })
            .collect();
        app.snapshot
            .workspace
            .selected_profiles
            .insert("mihomo".into(), "sample-1".into());
        app.snapshot.workspace.endpoints=vec![serde_json::from_value(serde_json::json!({"id":"example","name":"Example endpoint","type":"vless","server":"example.com","port":443})).unwrap()];
        app.snapshot.workspace.active_endpoint = Some("example".into());
        app.paths.root = "C:\\URTW-preview".into();
        app.snapshot.settings.profiles = (1..=8)
            .map(|i| format!("C:\\URTW-preview\\profiles\\profile-{i}.yaml").into())
            .collect();
        app.snapshot.settings.active_config = app.snapshot.settings.profiles.first().cloned();
        app.snapshot.mihomo = Service {
            installed: true,
            running: true,
            managed: true,
            listening: true,
            pid: Some(1200),
            memory_mb: 36,
            uptime_secs: Some(4800),
        };
        app.snapshot.telegram = Service {
            installed: true,
            ..Service::default()
        };
        app.snapshot.zapret = Service {
            installed: true,
            running: true,
            listening: true,
            pid: Some(1400),
            ..Service::default()
        };
        for c in &app.snapshot.cores {
            app.snapshot.core_states.insert(
                c.id.clone(),
                crate::workspace::CoreState {
                    service: if c.id == "mihomo" {
                        app.snapshot.mihomo.clone()
                    } else {
                        Service {
                            installed: true,
                            ..Service::default()
                        }
                    },
                    running_config: (c.id == "mihomo")
                        .then(|| r"C:\URTW-preview\profiles\profile-1.yaml".into()),
                    running_port: (c.id == "mihomo").then_some(17890),
                },
            );
        }
        for (i, t) in app.diag_targets.iter_mut().enumerate() {
            t.status = if i % 3 == 0 {
                DiagStatus::Failed("Timeout".into())
            } else {
                DiagStatus::Http {
                    code: if i % 3 == 1 { 200 } else { 403 },
                    ms: 30 + i as u64 * 8,
                }
            };
        }
    } else {
        app.snapshot = system::inspect(&app.paths);
    }
    let mut terminal = Terminal::new(TestBackend::new(opts.width, opts.height))?;
    terminal.draw(|f| ui::render(f, &app))?;
    let cells:Vec<_>=terminal.backend().buffer().content().iter().map(|c|serde_json::json!({"text":c.symbol(),"fg":color(c.fg,"#f1f5f9"),"bg":color(c.bg,"#0f172a")})).collect();
    let output = opts.snapshot.as_ref().context("Snapshot path missing")?;
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        output,
        serde_json::to_vec(
            &serde_json::json!({"width":opts.width,"height":opts.height,"cells":cells}),
        )?,
    )?;
    Ok(())
}

fn key(app: &mut App, key: KeyCode) -> bool {
    use serde_json::json;
    if let Some(input) = &mut app.input {
        match key {
            KeyCode::Esc => app.input = None,
            KeyCode::Tab | KeyCode::Down => {
                input.index = (input.index + 1) % input.fields.len().max(1)
            }
            KeyCode::BackTab | KeyCode::Up => {
                input.index =
                    (input.index + input.fields.len().max(1) - 1) % input.fields.len().max(1)
            }
            KeyCode::Backspace => {
                if let Some(f) = input.fields.get_mut(input.index) {
                    f.value.pop();
                }
            }
            KeyCode::Char(c) if !c.is_control() => {
                if let Some(f) = input.fields.get_mut(input.index)
                    && f.value.len() < 16384
                {
                    f.value.push(c);
                }
            }
            KeyCode::Delete => {
                if let Some(f) = input.fields.get_mut(input.index) {
                    f.value.clear();
                }
            }
            KeyCode::Enter => app.submit(),
            _ => {}
        }
        return false;
    }
    let idx = app.selected[app.tab.index()];
    let core = app.snapshot.workspace.active_core.clone();
    let profile = app
        .snapshot
        .workspace
        .core_profiles()
        .get(idx)
        .map(|p| (*p).clone());
    let rule_index = idx.checked_sub(app.snapshot.workspace.standards.len());
    match key {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
            if app.operation.is_some() {
                app.notify("Wait for the running operation to finish", true)
            } else {
                return true;
            }
        }
        KeyCode::Char(c @ '1'..='9') => app.tab = Tab::ALL[c as usize - '1' as usize],
        KeyCode::Char('0') => app.tab = Tab::Settings,
        KeyCode::F(n @ 1..=10) => app.tab = Tab::ALL[n as usize - 1],
        KeyCode::Tab => app.move_tab(1),
        KeyCode::BackTab => app.move_tab(-1),
        KeyCode::Up => app.move_selection(-1),
        KeyCode::Down => app.move_selection(1),
        KeyCode::PageUp => app.move_selection(-8),
        KeyCode::PageDown => app.move_selection(8),
        KeyCode::Home => app.selected[app.tab.index()] = 0,
        KeyCode::End => app.selected[app.tab.index()] = app.list_len().saturating_sub(1),
        KeyCode::Char('f' | 'F') => app.form(
            "Select native config or WireGuard file",
            json!({"action":"SelectFile","core":core}),
            vec![App::field("path", "Absolute file path", "", false)],
        ),
        KeyCode::Char('n' | 'N') => app.pick(false),
        KeyCode::Char('d' | 'D') => {
            app.tab = Tab::Diagnostics;
            app.diagnostics()
        }
        KeyCode::Char('m' | 'M') if app.tab == Tab::Diagnostics => {
            if !app.diag_running {
                app.diag_via_proxy = !app.diag_via_proxy;
            }
        }
        KeyCode::Char('s' | 'S') => {
            if app.tab == Tab::Telegram {
                app.action(Action::StartTelegram)
            } else if matches!(app.tab, Tab::Overview | Tab::Profiles | Tab::Cores) {
                app.core_request("Start", "Starting selected core")
            }
        }
        KeyCode::Char('x' | 'X') => {
            if app.tab == Tab::Telegram {
                app.action(Action::StopTelegram)
            } else if matches!(app.tab, Tab::Overview | Tab::Profiles | Tab::Cores) {
                app.core_request("Stop", "Stopping owned core")
            }
        }
        KeyCode::Char('y' | 'Y') if app.tab == Tab::Overview => {
            app.core_request("OpenLegacy", "Opening original legacy manager")
        }
        KeyCode::Char('p' | 'P') if app.tab == Tab::Overview => {
            if app.snapshot.proxy_enabled {
                app.action(Action::DisableProxy)
            } else {
                app.core_request("EnableProxy", "Enabling Windows proxy")
            }
        }
        KeyCode::Char('i' | 'I') => match app.tab {
            Tab::Telegram => app.action(Action::Install(Component::Telegram)),
            Tab::Zapret => app.action(Action::Install(Component::Zapret)),
            Tab::Profiles | Tab::Cores => app.core_request("Install", "Installing selected core"),
            Tab::Components => app.action(Action::Install(Component::ALL[idx])),
            _ => {}
        },
        KeyCode::Char('g' | 'G') => {
            if app.tab == Tab::Telegram {
                app.action(Action::OpenTelegram)
            } else if matches!(app.tab, Tab::Profiles | Tab::Routing | Tab::Cores) {
                app.core_request("Generate", "Generating a new private profile")
            }
        }
        KeyCode::Char('v' | 'V') if app.tab == Tab::Profiles => {
            app.core_request("Validate", "Validating selected profile")
        }
        KeyCode::Char('c' | 'C') if app.tab == Tab::Profiles => {
            if let Some(p) = profile {
                app.request(
                    json!({"action":"Clone","id":p.id}),
                    "Copying profile for private editing",
                )
            }
        }
        KeyCode::Char('b' | 'B') if app.tab == Tab::Cores => app.form(
            "Install trusted local core binary",
            json!({"action":"InstallBinary","core":core}),
            vec![App::field("path", "Absolute executable path", "", false)],
        ),
        KeyCode::Char('a' | 'A') if app.tab == Tab::Settings => app.form(
            "Toggle selected core autostart at Windows logon",
            json!({"action":"Autostart"}),
            vec![],
        ),
        KeyCode::Char('r' | 'R') if app.tab == Tab::Endpoints => {
            if let Some(e) = app.snapshot.workspace.endpoints.get(idx) {
                app.form(
                    "Rename endpoint",
                    json!({"action":"RenameEndpoint","id":e.id}),
                    vec![App::field("name", "Name", e.name.clone(), false)],
                );
            }
        }
        KeyCode::Char('r' | 'R') if app.tab == Tab::Routing => {
            if let Some(s) = app.snapshot.workspace.standards.get(idx) {
                app.form(
                    "Rename standard",
                    json!({"action":"RenameStandard","id":s.id}),
                    vec![App::field("name", "Name", s.name.clone(), false)],
                );
            }
        }
        KeyCode::Char('e' | 'E') if app.tab == Tab::Endpoints => {
            if let Some(e) = app.snapshot.workspace.endpoints.get(idx) {
                app.form(
                    "Replace endpoint credentials privately",
                    json!({"action":"ReplaceEndpoint","id":e.id}),
                    vec![App::field("link", "New complete share link", "", true)],
                );
            }
        }
        KeyCode::Char('r' | 'R') if app.tab == Tab::Profiles => {
            if let Some(p) = profile {
                app.form(
                    "Rename profile",
                    json!({"action":"RenameProfile","id":p.id}),
                    vec![App::field("name", "Name", p.name, false)],
                )
            }
        }
        KeyCode::Char('l' | 'L') => {
            if app.tab == Tab::Telegram {
                app.action(Action::CopyTelegramLink)
            } else {
                app.core_request("OpenLogs", "Opening private core log")
            }
        }
        KeyCode::Char('o' | 'O') => {
            if app.tab == Tab::Telegram {
                app.action(Action::EditTelegram)
            } else if app.tab == Tab::Settings {
                app.core_request("OpenFolder", "Opening data folder")
            } else if app.tab == Tab::Routing {
                app.core_request("OpenPac", "Opening browser PAC")
            } else {
                app.core_request("OpenProfile", "Opening selected native profile")
            }
        }
        KeyCode::Char('a' | 'A') => match app.tab {
            Tab::Endpoints => app.form(
                "Import endpoint link (stored privately)",
                json!({"action":"AddEndpoint"}),
                vec![App::field(
                    "link",
                    "SOCKS5 / HTTP(S) / VLESS / Trojan link",
                    "",
                    true,
                )],
            ),
            Tab::Routing => app.form(
                "New routing standard",
                json!({"action":"AddStandard"}),
                vec![App::field("name", "Name", "", false)],
            ),
            Tab::Cores => app.form(
                "Import custom native core adapter",
                json!({"action":"ImportAdapter"}),
                vec![App::field("path", "Absolute adapter JSON path", "", false)],
            ),
            _ => {}
        },
        KeyCode::Char('w' | 'W') if app.tab == Tab::Endpoints => app.form(
            "Import WireGuard endpoint",
            json!({"action":"AddEndpoint"}),
            vec![App::field("path", "Absolute .conf path", "", false)],
        ),
        KeyCode::Char('e' | 'E') => match app.tab {
            Tab::Telegram => app.form(
                "Telegram local port",
                json!({"action":"SetTGPort"}),
                vec![App::field(
                    "port",
                    "Port",
                    app.snapshot.settings.tg_port.to_string(),
                    false,
                )],
            ),
            Tab::Settings | Tab::Cores => {
                let opt = app
                    .snapshot
                    .workspace
                    .core_options
                    .get(&core)
                    .cloned()
                    .unwrap_or(crate::workspace::CoreOptions {
                        port: 17890,
                        dns: vec!["1.1.1.1".into()],
                        dns_strategy: "prefer_ipv4".into(),
                        ipv6: false,
                        tun: false,
                        log_level: "warning".into(),
                    });
                app.form(
                    "Core options (G generates a new profile)",
                    json!({"action":"CoreOptions","core":core}),
                    vec![
                        App::field("port", "Local port", opt.port.to_string(), false),
                        App::field(
                            "dns",
                            "DNS servers, comma separated",
                            opt.dns.join(", "),
                            false,
                        ),
                        App::field(
                            "dns_strategy",
                            "prefer_ipv4 / prefer_ipv6 / ipv4_only / ipv6_only",
                            opt.dns_strategy,
                            false,
                        ),
                        App::field("ipv6", "IPv6: true / false", opt.ipv6.to_string(), false),
                        App::field("tun", "TUN: true / false", opt.tun.to_string(), false),
                        App::field(
                            "log_level",
                            "debug / info / warning / error",
                            opt.log_level,
                            false,
                        ),
                    ],
                );
            }
            Tab::Routing => app.form(
                "Add ordered routing rule",
                json!({"action":"AddRule"}),
                vec![App::field(
                    "rule",
                    "domain example.com proxy | process app.exe direct | ip CIDR block",
                    "",
                    false,
                )],
            ),
            _ => {}
        },
        KeyCode::Char('b' | 'B') if app.tab == Tab::Routing => {
            let value = app
                .snapshot
                .workspace
                .standard()
                .map(|s| s.browser_domains.join(", "))
                .unwrap_or_default();
            app.form(
                "Browser split domains",
                json!({"action":"BrowserDomains"}),
                vec![App::field(
                    "domains",
                    "Comma separated domains routed through this core",
                    value,
                    false,
                )],
            );
        }
        KeyCode::Char('t' | 'T') if app.tab == Tab::Routing => app.form(
            "Routing fallback",
            json!({"action":"DefaultRoute"}),
            vec![App::field(
                "value",
                "direct / proxy / block",
                "direct",
                false,
            )],
        ),
        KeyCode::Char('u' | 'U') if app.tab == Tab::Routing => {
            if let Some(index) = rule_index {
                app.request(
                    json!({"action":"MoveRule","index":index,"delta":-1}),
                    "Moving rule up",
                )
            }
        }
        KeyCode::Char('j' | 'J') if app.tab == Tab::Routing => {
            if let Some(index) = rule_index {
                app.request(
                    json!({"action":"MoveRule","index":index,"delta":1}),
                    "Moving rule down",
                )
            }
        }
        KeyCode::Delete => {
            let req = match app.tab {
                Tab::Profiles => profile.map(|p| json!({"action":"RemoveProfile","id":p.id})),
                Tab::Endpoints => app
                    .snapshot
                    .workspace
                    .endpoints
                    .get(idx)
                    .map(|e| json!({"action":"RemoveEndpoint","id":e.id})),
                Tab::Routing => rule_index
                    .map(|i| json!({"action":"RemoveRule","index":i}))
                    .or_else(|| {
                        app.snapshot
                            .workspace
                            .standards
                            .get(idx)
                            .map(|s| json!({"action":"RemoveStandard","id":s.id}))
                    }),
                _ => None,
            };
            if let Some(r) = req {
                app.form(
                    "Confirm removal from library (files are retained)",
                    r,
                    vec![],
                )
            }
        }
        KeyCode::Char('h' | 'H') if app.tab == Tab::Settings => app.form(
            "Choose data folder (existing processes remain active)",
            json!({"action":"Storage"}),
            vec![App::field(
                "path",
                "Absolute private data folder",
                app.paths.root.display().to_string(),
                false,
            )],
        ),
        KeyCode::Char('k' | 'K') if app.tab == Tab::Settings => app.pick(true),
        KeyCode::Char('b' | 'B') if app.tab == Tab::Settings => {
            app.core_request("Backup", "Creating private workspace backup")
        }
        KeyCode::Char('r' | 'R') if app.tab == Tab::Settings => app.form(
            "Restore private backup (all managed cores must be stopped)",
            json!({"action":"Restore"}),
            vec![App::field("path", "Absolute backup folder", "", false)],
        ),
        KeyCode::Enter => match app.tab {
            Tab::Profiles => {
                if let Some(p) = profile {
                    app.request(
                        json!({"action":"SelectProfile","id":p.id}),
                        "Selecting profile",
                    )
                }
            }
            Tab::Cores => {
                if let Some(c) = app.snapshot.cores.get(idx) {
                    app.request(json!({"action":"SelectCore","id":c.id}), "Selecting core")
                }
            }
            Tab::Endpoints => {
                if let Some(e) = app.snapshot.workspace.endpoints.get(idx) {
                    app.request(
                        json!({"action":"SelectEndpoint","id":e.id}),
                        "Selecting endpoint",
                    )
                }
            }
            Tab::Routing => {
                if let Some(s) = app.snapshot.workspace.standards.get(idx) {
                    app.request(
                        json!({"action":"SelectStandard","id":s.id}),
                        "Selecting routing standard",
                    )
                }
            }
            Tab::Diagnostics => app.diagnostics(),
            Tab::Telegram => app.action(Action::OpenTelegram),
            Tab::Zapret => app.action(Action::OpenZapret),
            Tab::Components => app.action(Action::Install(Component::ALL[idx])),
            _ => {}
        },
        _ => {}
    }
    false
}

#[tokio::main]
async fn main() -> Result<()> {
    let Some(opts) = options()? else {
        return Ok(());
    };
    let paths = Paths::discover(opts.root.clone(), opts.assets.clone())?;
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);
    let mut app = App::new(paths.clone(), tx.clone());
    if opts.snapshot.is_some() {
        return snapshot(app, &opts);
    }
    if !cfg!(windows) {
        bail!("The interactive dashboard requires Windows");
    }
    app.refresh();
    let mut last_refresh = std::time::Instant::now();
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(
        stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    loop {
        if last_refresh.elapsed() >= Duration::from_secs(2) {
            app.refresh();
            last_refresh = std::time::Instant::now();
        }
        while let Ok(event) = rx.try_recv() {
            app.event(event);
        }
        terminal.draw(|f| ui::render(f, &app))?;
        app.ticks = app.ticks.wrapping_add(1);
        if event::poll(Duration::from_millis(60))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if key(&mut app, k.code) {
                        break;
                    }
                }
                Event::Paste(text) => {
                    if let Some(input) = &mut app.input
                        && let Some(field) = input.fields.get_mut(input.index)
                    {
                        let clean: String = text
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(16384usize.saturating_sub(field.value.len()))
                            .collect();
                        field.value.push_str(&clean);
                    }
                }
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollUp => app.move_selection(-2),
                    MouseEventKind::ScrollDown => app.move_selection(2),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    terminal.show_cursor()?;
    Ok(())
}
