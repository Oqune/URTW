use crate::{app::App, model::*, theme::Theme};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Tabs, Wrap,
    },
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.into();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}
fn panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::BORDER))
        .padding(Padding::new(1, 1, 0, 0))
}
fn sections(
    area: Rect,
    constraints: impl IntoIterator<Item = Constraint>,
    direction: Direction,
) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(direction)
        .constraints(constraints)
        .spacing(1)
        .split(area)
}
fn body(f: &mut Frame, area: Rect, title: &str, text: Vec<Line<'static>>) {
    f.render_widget(
        Paragraph::new(text)
            .block(panel(format!(" {title} ")))
            .wrap(Wrap { trim: false }),
        area,
    );
}
fn line(label: &str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(Theme::TEXT_MUTED)),
        Span::styled(value.into(), Style::default().fg(Theme::TEXT_PRIMARY)),
    ])
}
fn status_style(service: &Service) -> Style {
    Style::default().fg(if service.running && service.listening {
        if service.managed {
            Theme::SUCCESS
        } else {
            Theme::WARNING
        }
    } else if service.running || service.listening {
        Theme::WARNING
    } else {
        Theme::TEXT_MUTED
    })
}
fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(Theme::BG_CARD).fg(Theme::TEXT_PRIMARY)),
        area,
    );
    if area.width < 60 || area.height < 18 {
        f.render_widget(
            Paragraph::new("Resize to at least 60 x 18\nQ / Esc: quit")
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            centered(40, 3, area),
        );
        return;
    }
    if !app.initialized {
        let box_area = centered(52, 7, area);
        body(
            f,
            box_area,
            "URT",
            vec![
                Line::from(""),
                Line::from("Reading local component status...").fg(Theme::ACCENT),
                Line::from("Your routing remains under your control."),
            ],
        );
        return;
    }
    let rows = sections(
        area.inner(Margin {
            horizontal: 1,
            vertical: if area.height < 24 { 0 } else { 1 },
        }),
        [
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(3),
        ],
        Direction::Vertical,
    );
    let compact = area.width < 100;
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let title = if compact {
                ["Home", "Files", "Tests", "TG", "DPI", "Tools"][i]
            } else {
                t.title()
            };
            Line::from(format!("{} {title}", i + 1))
        })
        .collect();
    let tabs = Tabs::new(titles)
        .block(panel(format!(
            " URT v{}{} ",
            env!("CARGO_PKG_VERSION"),
            if app.preview { " / sample preview" } else { "" }
        )))
        .select(app.tab.index())
        .padding("", " ")
        .divider("│")
        .style(Style::default().fg(Theme::TEXT_MUTED))
        .highlight_style(Style::default().fg(Theme::ACCENT).bold());
    f.render_widget(tabs, rows[0]);
    match app.tab {
        Tab::Overview => overview(f, app, rows[1]),
        Tab::Profiles => profiles(f, app, rows[1]),
        Tab::Diagnostics => diagnostics(f, app, rows[1]),
        Tab::Telegram => telegram(f, app, rows[1]),
        Tab::Zapret => zapret(f, app, rows[1]),
        Tab::Components => components(f, app, rows[1]),
    }
    footer(f, app, rows[2]);
    if let Some(input) = &app.port_input {
        let popup = centered(48, 7, area);
        f.render_widget(Clear, popup);
        body(
            f,
            popup,
            "Telegram local port",
            vec![
                Line::from("Enter a port from 1024 to 65535."),
                Line::from(format!("> {input}_")).fg(Theme::ACCENT),
                Line::from("Stop the managed proxy before changing it."),
                Line::from("Enter: save     Esc: cancel").fg(Theme::TEXT_MUTED),
            ],
        );
    }
}

fn overview(f: &mut Frame, app: &App, area: Rect) {
    let block = panel(" Components & routing ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut rows = Vec::new();
    for (name, svc) in [
        ("Mihomo", &app.snapshot.mihomo),
        ("TG WS Proxy", &app.snapshot.telegram),
        ("Zapret", &app.snapshot.zapret),
    ] {
        let info = svc.pid.map_or_else(
            || "—".into(),
            |pid| {
                format!(
                    "PID {pid} / {} MB / {} min",
                    svc.memory_mb,
                    svc.uptime_secs / 60
                )
            },
        );
        rows.push(
            Row::new(vec![name.to_string(), svc.label().into(), info]).style(status_style(svc)),
        );
    }
    rows.push(Row::new(vec![
        "Windows proxy".into(),
        if app.snapshot.proxy_enabled {
            "Enabled".into()
        } else {
            "Disabled".into()
        },
        if app.snapshot.proxy_enabled {
            app.snapshot.proxy_server.clone()
        } else {
            "System route".into()
        },
    ]));
    let split = Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).split(inner);
    let widths = if inner.width >= 90 {
        vec![
            Constraint::Length(15),
            Constraint::Length(24),
            Constraint::Min(1),
        ]
    } else {
        vec![
            Constraint::Length(14),
            Constraint::Length(22),
            Constraint::Min(1),
        ]
    };
    f.render_widget(
        Table::new(rows, widths)
            .header(
                Row::new(["Component", "State", "Process"])
                    .fg(Theme::ACCENT)
                    .bold(),
            )
            .column_spacing(1),
        split[0],
    );
    let config = app.snapshot.settings.active_config.as_ref().map_or_else(
        || "None — press F to choose".into(),
        |p| p.display().to_string(),
    );
    let mut info = vec![
        line(
            "Config",
            fit(&config, inner.width.saturating_sub(8) as usize),
        ),
        line(
            "Runtime",
            fit(
                &app.paths.root.display().to_string(),
                inner.width.saturating_sub(9) as usize,
            ),
        ),
    ];
    if let Some(error) = &app.snapshot.error {
        info.insert(0, Line::from(error.clone()).fg(Theme::DANGER));
    }
    if area.height >= 15 {
        info.push(Line::from(""));
        info.push(
            Line::from("External = a process or listener outside this URT runtime.")
                .fg(Theme::WARNING),
        );
        info.push(Line::from(
            "Selecting a profile does not apply it to the running core.",
        ));
        info.push(Line::from(
            "Services keep running when you close this dashboard.",
        ));
    }
    f.render_widget(Paragraph::new(info).wrap(Wrap { trim: false }), split[1]);
}

fn scrollbar(f: &mut Frame, area: Rect, total: usize, visible: usize, start: usize) {
    if total > visible && visible > 0 {
        let mut state = ScrollbarState::new(total)
            .viewport_content_length(visible)
            .position(start);
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            area.inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut state,
        );
    }
}

fn profiles(f: &mut Frame, app: &App, area: Rect) {
    let entries = &app.snapshot.settings.profiles;
    if entries.is_empty() {
        body(
            f,
            area,
            "Configurations",
            vec![
                Line::from("No configuration selected.").fg(Theme::ACCENT),
                Line::from(""),
                Line::from("F: choose a Mihomo .yaml/.yml or WireGuard .conf file."),
                Line::from("YAML files stay in their original location."),
                Line::from("WireGuard import creates a private runtime YAML profile."),
                Line::from("Select and validate first; S starts Mihomo separately."),
            ],
        );
        return;
    }
    let cols = if area.width >= 110 {
        sections(
            area,
            [Constraint::Percentage(56), Constraint::Percentage(44)],
            Direction::Horizontal,
        )
    } else {
        Layout::horizontal([Constraint::Percentage(100)]).split(area)
    };
    let selected = app.selected[Tab::Profiles.index()].min(entries.len().saturating_sub(1));
    let block = panel(format!(
        " Configurations ({}/{}) ",
        selected + 1,
        entries.len()
    ));
    let height = block.inner(cols[0]).height.saturating_sub(1) as usize;
    let start = viewport_start(selected, entries.len(), height);
    let rows = entries.iter().skip(start).take(height).map(|path| {
        let active = app.snapshot.settings.active_config.as_ref() == Some(path);
        Row::new(vec![
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            if !app.preview && !path.is_file() {
                "Missing".into()
            } else if active {
                "Selected".into()
            } else {
                "Available".into()
            },
        ])
    });
    let table = Table::new(rows, [Constraint::Min(12), Constraint::Length(10)])
        .block(block)
        .header(Row::new(["File", "Selection"]).fg(Theme::ACCENT).bold())
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(Theme::BG_SELECTED)
                .fg(Theme::TEXT_PRIMARY),
        )
        .highlight_symbol("› ");
    let mut state = TableState::default().with_selected(Some(selected.saturating_sub(start)));
    f.render_stateful_widget(table, cols[0], &mut state);
    scrollbar(f, cols[0], entries.len(), height, start);
    if cols.len() > 1 {
        let running = app
            .snapshot
            .running_config
            .as_ref()
            .map_or_else(|| "Not managed here".into(), |p| p.display().to_string());
        body(
            f,
            cols[1],
            "Selection details",
            vec![
                line("File", entries[selected].display().to_string()),
                Line::from(""),
                line("Running config", running),
                Line::from(""),
                Line::from("Enter: select and validate"),
                Line::from("F: browse for another file"),
                Line::from("S / X: start / stop managed Mihomo"),
                Line::from("YAML selection leaves the original file unchanged.")
                    .fg(Theme::TEXT_MUTED),
            ],
        );
    }
}

fn diagnostics(f: &mut Frame, app: &App, area: Rect) {
    let rows = sections(
        area,
        [Constraint::Length(3), Constraint::Min(3)],
        Direction::Vertical,
    );
    let done = app
        .diag_targets
        .iter()
        .filter(|t| t.status.finished())
        .count();
    let ok = app
        .diag_targets
        .iter()
        .filter(|t| t.status.successful())
        .count();
    let pct = completed_percent(done, app.diag_targets.len());
    let mode = if app.diag_via_proxy {
        "Mihomo inbound"
    } else {
        "System route"
    };
    let gauge = Gauge::default()
        .block(panel(format!(" Diagnostic progress / {mode} ")))
        .gauge_style(Style::default().fg(Theme::ACCENT).bg(Theme::BG_SELECTED))
        .percent(pct)
        .label(format!(
            "{pct}% · {done}/{} completed · {ok} HTTP OK",
            app.diag_targets.len()
        ));
    f.render_widget(gauge, rows[0]);
    let block = panel(if area.width < 80 {
        " HTTP results / denials are responses "
    } else {
        " HTTP results (HTTP 403 is a response, not a transport failure) "
    });
    let visible = block.inner(rows[1]).height.saturating_sub(1) as usize;
    let selected =
        app.selected[Tab::Diagnostics.index()].min(app.diag_targets.len().saturating_sub(1));
    let start = viewport_start(selected, app.diag_targets.len(), visible);
    let table_rows = app.diag_targets.iter().skip(start).take(visible).map(|t| {
        let color = match &t.status {
            DiagStatus::Http {
                code: 200..=399, ..
            } => Theme::SUCCESS,
            DiagStatus::Http { .. } => Theme::WARNING,
            DiagStatus::Failed(_) => Theme::DANGER,
            DiagStatus::Running => Theme::ACCENT,
            DiagStatus::Pending => Theme::TEXT_MUTED,
        };
        Row::new(vec![t.name.to_owned(), t.status.text()]).fg(color)
    });
    let table = Table::new(table_rows, [Constraint::Min(12), Constraint::Length(26)])
        .block(block)
        .header(
            Row::new(["Target", "Result / elapsed time"])
                .fg(Theme::ACCENT)
                .bold(),
        )
        .column_spacing(2)
        .row_highlight_style(Style::default().bg(Theme::BG_SELECTED))
        .highlight_symbol("› ");
    let mut state = TableState::default().with_selected(Some(selected.saturating_sub(start)));
    f.render_stateful_widget(table, rows[1], &mut state);
    scrollbar(f, rows[1], app.diag_targets.len(), visible, start);
}

fn telegram(f: &mut Frame, app: &App, area: Rect) {
    let service = &app.snapshot.telegram;
    let mut text = vec![
        Line::from(service.label()).style(status_style(service).bold()),
        line(
            "Local MTProto endpoint",
            format!("127.0.0.1:{}", app.snapshot.settings.tg_port),
        ),
        line("Secret", "Private; L copies the connection link"),
        Line::from(""),
    ];
    if area.height >= 15 {
        text.extend([
            Line::from("Telegram Desktop → local MTProto → WebSocket → Telegram"),
            Line::from(""),
            Line::from("S: start the official portable tray application"),
            Line::from("X: stop the process started by this runtime"),
            Line::from("L: copy link     G: open link in Telegram"),
            Line::from("E: change local port (proxy must be stopped)"),
            Line::from("O: edit private configuration in Notepad"),
            Line::from("I: install the pinned upstream release"),
            Line::from("Advanced WS / DC / Cloudflare settings live in the upstream tray.")
                .fg(Theme::TEXT_MUTED),
        ]);
    } else {
        text.push(Line::from(
            "S/X start/stop · L link · E port · O settings · I install",
        ));
    }
    body(f, area, "TG WS Proxy", text);
}

fn zapret(f: &mut Frame, app: &App, area: Rect) {
    let path = app.paths.root.join("tools/zapret/service.bat");
    let mut text = vec![
        line("Observed process", app.snapshot.zapret.label()),
        line("Manager", path.display().to_string()),
        Line::from(""),
        Line::from("Enter: open the official service.bat manager").fg(Theme::ACCENT),
        Line::from("I: install the pinned package"),
    ];
    if area.height >= 14 {
        text.extend([
            Line::from(""),
            Line::from("Choose, install and remove strategies in the upstream manager."),
            Line::from("URT does not replace the selected strategy or delete services."),
            Line::from("Zapret / WinDivert currently requires x64 Windows."),
            Line::from("Administrator access is needed for service management.")
                .fg(Theme::TEXT_MUTED),
        ]);
    }
    body(f, area, "Zapret service manager", text);
}

fn components(f: &mut Frame, app: &App, area: Rect) {
    let block = panel(" Verified upstream components ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let sections = Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).split(inner);
    let rows = Component::ALL.into_iter().map(|c| {
        let installed = app
            .snapshot
            .installed_versions
            .get(c.id())
            .cloned()
            .unwrap_or_else(|| "Not tracked".into());
        Row::new(vec![
            c.title().to_owned(),
            installed,
            app.paths.pinned_version(c),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Min(14),
            Constraint::Length(15),
            Constraint::Length(15),
        ],
    )
    .column_spacing(1)
    .header(
        Row::new(["Component", "Installed", "Pinned"])
            .fg(Theme::ACCENT)
            .bold(),
    )
    .row_highlight_style(Style::default().bg(Theme::BG_SELECTED))
    .highlight_symbol("› ");
    let mut state =
        TableState::default().with_selected(Some(app.selected[Tab::Components.index()]));
    f.render_stateful_widget(table, sections[0], &mut state);
    let hint = if area.height >= 14 {
        "I / Enter: install the selected pinned component.\n\nDownloads come from official release URLs and are checked against\nthe SHA-256 digest in components.lock.json before installation.\nRunning managed components must be stopped before replacement.\nInstallation does not start components or change system routing."
    } else {
        "I / Enter: install selected; explicit start afterwards"
    };
    f.render_widget(
        Paragraph::new(hint)
            .fg(Theme::TEXT_MUTED)
            .wrap(Wrap { trim: false }),
        sections[1],
    );
}

fn footer(f: &mut Frame, app: &App, area: Rect) {
    let width = area.width.saturating_sub(4) as usize;
    if let Some(op) = &app.operation {
        let block = panel(format!(" {} ", fit(&op.label, width)));
        if let Some(progress) = op.progress {
            f.render_widget(
                Gauge::default()
                    .block(block)
                    .percent(progress.min(100))
                    .gauge_style(Style::default().fg(Theme::ACCENT).bg(Theme::BG_SELECTED))
                    .label(fit(
                        &format!("{}% · {}", progress.min(100), op.detail),
                        width,
                    )),
                area,
            );
        } else {
            let spinner = ["|", "/", "-", "\\"][(app.ticks / 3 % 4) as usize];
            f.render_widget(
                Paragraph::new(fit(&format!("{spinner} {}", op.detail), width))
                    .fg(Theme::ACCENT)
                    .block(block),
                area,
            );
        }
        return;
    }
    if let Some((message, expiry, error)) = &app.notice
        && std::time::Instant::now() < *expiry
    {
        f.render_widget(
            Paragraph::new(fit(message, width))
                .fg(if *error {
                    Theme::DANGER
                } else {
                    Theme::SUCCESS
                })
                .block(panel(if *error { " Error " } else { " Status " })),
            area,
        );
        return;
    }
    let keys = match app.tab {
        Tab::Overview => "F Config · S Start Mihomo · X Stop · P Proxy · Q Quit",
        Tab::Profiles => "↑↓ Select · Enter Apply selection · F File · S/X Start/Stop · Q Quit",
        Tab::Diagnostics => "↑↓ Scroll · D Run tests · M Change path · Tab Tabs · Q Quit",
        Tab::Telegram => "S/X Start/Stop · L Link · G Connect · E Port · O Config · I Install · Q",
        Tab::Zapret => "Enter Service manager · I Install · Tab Tabs · Q Quit",
        Tab::Components => "↑↓ Select · I/Enter Install pinned · Tab Tabs · Q Quit",
    };
    let compact = match app.tab {
        Tab::Profiles => "↑↓ Select · Enter Choose · F File · S/X Run/Stop · Q",
        Tab::Diagnostics => "↑↓ Scroll · D Test · M Path · Tab Tabs · Q Quit",
        Tab::Telegram => "S/X Run/Stop · L Link · G TG · E Port · O Edit · I · Q",
        _ => keys,
    };
    f.render_widget(
        Paragraph::new(fit(if area.width < 95 { compact } else { keys }, width))
            .block(panel(" Actions ")),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::Paths;
    use ratatui::{Terminal, backend::TestBackend};
    fn app() -> App {
        let (tx, _) = tokio::sync::mpsc::channel(16);
        let mut app = App::new(
            Paths {
                root: "runtime".into(),
                assets: ".".into(),
            },
            tx,
        );
        app.initialized = true;
        app
    }
    fn screen(app: &App, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }
    #[test]
    fn renders_every_tab_at_small_and_large_sizes() {
        for (w, h) in [
            (1, 1),
            (30, 8),
            (60, 18),
            (80, 24),
            (100, 24),
            (120, 30),
            (200, 60),
        ] {
            for tab in Tab::ALL {
                let mut app = app();
                app.tab = tab;
                app.snapshot.settings.profiles = (0..50)
                    .map(|i| format!("profiles/Профиль-{i}.yaml").into())
                    .collect();
                app.selected[1] = 49;
                for progress in [None, Some(0), Some(50), Some(100)] {
                    app.operation = Some(Operation {
                        label: "A long asynchronous operation".into(),
                        progress,
                        detail: "Measured progress".into(),
                    });
                    let text = screen(&app, w, h);
                    if w >= 60 && h >= 18 {
                        assert!(text.contains(&format!("URT v{}", env!("CARGO_PKG_VERSION"))));
                    }
                }
                app.port_input = Some("1443".into());
                let _ = screen(&app, w, h);
            }
        }
    }
    #[test]
    fn completed_diagnostics_reach_one_hundred_with_failures() {
        let mut app = app();
        app.tab = Tab::Diagnostics;
        for (i, t) in app.diag_targets.iter_mut().enumerate() {
            t.status = if i % 2 == 0 {
                DiagStatus::Http { code: 403, ms: 42 }
            } else {
                DiagStatus::Failed("Timeout".into())
            };
        }
        let text = screen(&app, 80, 24);
        assert!(text.contains("100%"));
        assert!(text.contains("10/10 completed"));
        assert!(text.contains("0 HTTP OK"));
    }
    #[test]
    fn unicode_truncation_respects_cells() {
        for width in 0..30 {
            assert!(fit("📁 Профиль 日本語 конфигурации", width).width() <= width);
        }
    }
    #[test]
    fn loading_screen_is_safe_at_minimum_size() {
        let mut app = app();
        app.initialized = false;
        assert!(screen(&app, 60, 18).contains("Reading local component status"));
    }
    #[test]
    fn no_fabricated_network_claims() {
        let text = screen(&app(), 120, 30);
        for phrase in ["CONNECTED", "Hetzner", "Metric: 5", "42ms", "OPTIMAL"] {
            assert!(!text.contains(phrase));
        }
    }
}
