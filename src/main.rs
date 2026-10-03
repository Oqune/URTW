mod app;
mod model;
mod network;
mod system;
mod theme;
mod ui;

use anyhow::{Context, Result, bail};
use app::{App, AppEvent};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
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
                    "URT {}\n\nWindows routing dashboard. Startup only reads local status.\n\nURT [--root ABSOLUTE_PATH] [--assets ABSOLUTE_PATH]\nURT --snapshot FILE.json [--width 120 --height 30 --tab 1] [--preview]\n\nRuntime: URT_HOME or %LOCALAPPDATA%\\URT. Assets: beside URT.exe.\nF: select a config; S/X: start/stop; 1..6 or Tab: navigation.\n--preview uses sample data and is available only with --snapshot.",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(None);
            }
            "--version" | "-V" => {
                println!("URT {}", env!("CARGO_PKG_VERSION"));
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
                    .context("--tab needs a number from 1 to 6")?
                    .parse::<usize>()?
                    .checked_sub(1)
                    .context("Tab numbers begin at 1")?
            }
            "--preview" => opts.preview = true,
            _ => bail!("Unknown option: {arg}; use --help"),
        }
    }
    if !(1..=500).contains(&opts.width) || !(1..=200).contains(&opts.height) || opts.tab >= 6 {
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
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
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
        app.paths.root = "C:\\URT-preview".into();
        app.snapshot.settings.profiles = (1..=8)
            .map(|i| format!("C:\\URT-preview\\profiles\\profile-{i}.yaml").into())
            .collect();
        app.snapshot.settings.active_config = app.snapshot.settings.profiles.first().cloned();
        app.snapshot.mihomo = Service {
            installed: true,
            running: true,
            managed: true,
            listening: true,
            pid: Some(1200),
            memory_mb: 36,
            uptime_secs: 4800,
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
    if let Some(input) = &mut app.port_input {
        match key {
            KeyCode::Esc => app.port_input = None,
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) if c.is_ascii_digit() && input.len() < 5 => input.push(c),
            KeyCode::Enter => {
                let port = input.parse::<u16>().ok().filter(|p| *p >= 1024);
                if let Some(p) = port {
                    app.port_input = None;
                    app.action(Action::SetTelegramPort(p));
                } else {
                    app.notify("Enter a port from 1024 to 65535", true);
                }
            }
            _ => {}
        }
        return false;
    }
    match key {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
            if app.operation.is_some() {
                app.notify("Wait for the running operation to finish", true);
            } else {
                return true;
            }
        }
        KeyCode::Char(c @ '1'..='6') => app.tab = Tab::ALL[c as usize - '1' as usize],
        KeyCode::F(n @ 1..=6) => app.tab = Tab::ALL[n as usize - 1],
        KeyCode::Tab => app.move_tab(1),
        KeyCode::BackTab => app.move_tab(-1),
        KeyCode::Up => app.move_selection(-1),
        KeyCode::Down => app.move_selection(1),
        KeyCode::PageUp => app.move_selection(-8),
        KeyCode::PageDown => app.move_selection(8),
        KeyCode::Home => app.selected[app.tab.index()] = 0,
        KeyCode::End => app.selected[app.tab.index()] = app.list_len().saturating_sub(1),
        KeyCode::Char('f' | 'F') => app.pick_config(),
        KeyCode::Char('d' | 'D') => {
            app.tab = Tab::Diagnostics;
            app.diagnostics();
        }
        KeyCode::Char('m' | 'M') if app.tab == Tab::Diagnostics => {
            if !app.diag_running {
                app.diag_via_proxy = !app.diag_via_proxy;
            }
        }
        KeyCode::Char('s' | 'S') => match app.tab {
            Tab::Overview | Tab::Profiles => app.action(Action::StartMihomo),
            Tab::Telegram => app.action(Action::StartTelegram),
            _ => {}
        },
        KeyCode::Char('x' | 'X') => match app.tab {
            Tab::Overview | Tab::Profiles => app.action(Action::StopMihomo),
            Tab::Telegram => app.action(Action::StopTelegram),
            _ => {}
        },
        KeyCode::Char('p' | 'P') if app.tab == Tab::Overview => {
            app.action(if app.snapshot.proxy_enabled {
                Action::DisableProxy
            } else {
                Action::EnableProxy
            })
        }
        KeyCode::Char('i' | 'I') => match app.tab {
            Tab::Telegram => app.action(Action::Install(Component::Telegram)),
            Tab::Zapret => app.action(Action::Install(Component::Zapret)),
            Tab::Profiles => app.action(Action::Install(Component::Mihomo)),
            Tab::Components => app.action(Action::Install(Component::ALL[app.selected[5]])),
            _ => {}
        },
        KeyCode::Char('l' | 'L') if app.tab == Tab::Telegram => {
            app.action(Action::CopyTelegramLink)
        }
        KeyCode::Char('g' | 'G') if app.tab == Tab::Telegram => app.action(Action::OpenTelegram),
        KeyCode::Char('o' | 'O') if app.tab == Tab::Telegram => app.action(Action::EditTelegram),
        KeyCode::Char('e' | 'E') if app.tab == Tab::Telegram => {
            app.port_input = Some(app.snapshot.settings.tg_port.to_string())
        }
        KeyCode::Enter => match app.tab {
            Tab::Profiles => {
                if let Some(p) = app.snapshot.settings.profiles.get(app.selected[1]).cloned() {
                    app.action(Action::SelectConfig(p));
                }
            }
            Tab::Diagnostics => app.diagnostics(),
            Tab::Telegram => app.action(Action::OpenTelegram),
            Tab::Zapret => app.action(Action::OpenZapret),
            Tab::Components => app.action(Action::Install(Component::ALL[app.selected[5]])),
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
    tokio::spawn(async move {
        loop {
            let paths = paths.clone();
            let snapshot = tokio::task::spawn_blocking(move || system::inspect(&paths)).await;
            if let Ok(snapshot) = snapshot
                && tx
                    .send(AppEvent::Snapshot(Box::new(snapshot)))
                    .await
                    .is_err()
            {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    loop {
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
