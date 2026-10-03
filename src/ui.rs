use crate::{app::App, model::*, theme::Theme};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
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
            "URTW",
            vec![
                Line::from(""),
                Line::from("Reading local component status...").fg(Theme::ACCENT),
                Line::from("Your routing remains under your control."),
            ],
        );
        return;
    }
    let outer = area.inner(Margin {
        horizontal: 1,
        vertical: u16::from(area.height >= 24),
    });
    let wide = area.width >= 110;
    let cols = Layout::horizontal(if wide {
        vec![Constraint::Length(21), Constraint::Min(1)]
    } else {
        vec![Constraint::Length(0), Constraint::Min(1)]
    })
    .spacing(u16::from(wide))
    .split(outer);
    let rows = Layout::vertical([
        Constraint::Length(if wide { 3 } else { 4 }),
        Constraint::Min(6),
        Constraint::Length(3),
    ])
    .spacing(1)
    .split(cols[1]);
    body(
        f,
        rows[0],
        &format!(
            "URTW v{}{}",
            env!("CARGO_PKG_VERSION"),
            if app.preview { " / sample preview" } else { "" }
        ),
        if wide {
            vec![
                Line::from(format!(
                    "{}  /  {}",
                    app.tab.title(),
                    app.snapshot.workspace.active_core
                ))
                .fg(Theme::ACCENT),
            ]
        } else {
            (0..2)
                .map(|row| {
                    Line::from(
                        (0..5)
                            .map(|col| {
                                let i = row * 5 + col;
                                let labels = [
                                    "Home", "Files", "Tests", "TG", "Zapret", "Tools", "Cores",
                                    "Nodes", "Rules", "Setup",
                                ];
                                Span::styled(
                                    format!("{} {:<6}", if i == 9 { 0 } else { i + 1 }, labels[i]),
                                    Style::default()
                                        .fg(if app.tab.index() == i {
                                            Theme::ACCENT
                                        } else {
                                            Theme::TEXT_MUTED
                                        })
                                        .add_modifier(if app.tab.index() == i {
                                            ratatui::style::Modifier::BOLD
                                        } else {
                                            ratatui::style::Modifier::empty()
                                        }),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect()
        },
    );
    if wide {
        let mut lines = vec![
            Line::from("URTA companion").fg(Theme::TEXT_MUTED),
            Line::from(""),
        ];
        for (i, tab) in Tab::ALL.iter().enumerate() {
            lines.push(
                Line::from(format!(
                    " {}  {}",
                    if i == 9 { 0 } else { i + 1 },
                    tab.title()
                ))
                .style(
                    Style::default()
                        .fg(if *tab == app.tab {
                            Theme::ACCENT
                        } else {
                            Theme::TEXT_MUTED
                        })
                        .bg(if *tab == app.tab {
                            Theme::BG_SELECTED
                        } else {
                            Theme::BG_CARD
                        }),
                ),
            );
            lines.push(Line::from(""));
        }
        body(f, cols[0], "Navigation", lines);
    }
    match app.tab {
        Tab::Overview => overview(f, app, rows[1]),
        Tab::Profiles => profiles(f, app, rows[1]),
        Tab::Diagnostics => diagnostics(f, app, rows[1]),
        Tab::Telegram => telegram(f, app, rows[1]),
        Tab::Zapret => zapret(f, app, rows[1]),
        Tab::Components => components(f, app, rows[1]),
        Tab::Cores => cores(f, app, rows[1]),
        Tab::Endpoints => endpoints(f, app, rows[1]),
        Tab::Routing => routing(f, app, rows[1]),
        Tab::Settings => settings(f, app, rows[1]),
    }
    footer(f, app, rows[2]);
    if let Some(input) = &app.input {
        let height = (input.fields.len() * 2 + 5).min(usize::from(area.height)) as u16;
        let popup = centered(94, height, area);
        f.render_widget(Clear, popup);
        let mut lines = vec![
            Line::from("Tab / Shift+Tab: field  ·  Delete: clear  ·  Enter: save  ·  Esc: cancel")
                .fg(Theme::TEXT_MUTED),
        ];
        for (i, field) in input.fields.iter().enumerate() {
            lines.push(Line::from(field.label).fg(Theme::TEXT_MUTED));
            let value = if field.secret {
                "•".repeat(field.value.chars().count().min(60))
            } else {
                field.value.clone()
            };
            lines.push(
                Line::from(fit(
                    &format!(
                        "{} {value}{}",
                        if input.index == i { "›" } else { " " },
                        if input.index == i { "_" } else { "" }
                    ),
                    popup.width.saturating_sub(6) as usize,
                ))
                .fg(if input.index == i {
                    Theme::ACCENT
                } else {
                    Theme::TEXT_PRIMARY
                }),
            );
        }
        if input.fields.is_empty() {
            lines.push(Line::from("Enter confirms. Esc cancels.").fg(Theme::WARNING));
        }
        body(f, popup, input.title, lines);
    }
}

fn overview(f: &mut Frame, app: &App, area: Rect) {
    let block = panel(" Components & routing ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut rows = Vec::new();
    let mut services: Vec<(&str, &Service)> = app
        .snapshot
        .cores
        .iter()
        .filter_map(|c| {
            app.snapshot
                .core_states
                .get(&c.id)
                .map(|s| (c.name.as_str(), &s.service))
        })
        .collect();
    if services.is_empty() {
        services.push(("Mihomo", &app.snapshot.mihomo));
    }
    services.extend([
        ("TG WS Proxy", &app.snapshot.telegram),
        ("Zapret", &app.snapshot.zapret),
    ]);
    for (name, svc) in services {
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
    let split = Layout::vertical([Constraint::Length(7), Constraint::Min(0)]).split(inner);
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
    let config = app
        .snapshot
        .workspace
        .selected_profile()
        .map(|p| &p.path)
        .map_or_else(
            || "None — press F to choose".into(),
            |p| p.display().to_string(),
        );
    let mut info = vec![
        line(
            "Endpoint",
            app.snapshot
                .workspace
                .endpoint()
                .map_or("None", |e| e.name.as_str()),
        ),
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
            Line::from("External = a process or listener outside this URTW runtime.")
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

fn library(
    f: &mut Frame,
    app: &App,
    area: Rect,
    title: &str,
    rows: Vec<Vec<String>>,
    headers: Vec<&str>,
    details: Vec<Line<'static>>,
) {
    let cols = if area.width >= 92 {
        sections(
            area,
            [Constraint::Percentage(55), Constraint::Percentage(45)],
            Direction::Horizontal,
        )
    } else {
        Layout::horizontal([Constraint::Percentage(100)]).split(area)
    };
    let block = panel(format!(" {title} "));
    let visible = block.inner(cols[0]).height.saturating_sub(1) as usize;
    let selected = app.selected[app.tab.index()].min(rows.len().saturating_sub(1));
    let start = viewport_start(selected, rows.len(), visible);
    let widths: Vec<_> = (0..headers.len())
        .map(|i| {
            if i == 0 {
                Constraint::Min(12)
            } else {
                Constraint::Length(12)
            }
        })
        .collect();
    let table = Table::new(
        rows.iter().skip(start).take(visible).cloned().map(Row::new),
        widths,
    )
    .header(Row::new(headers).fg(Theme::ACCENT).bold())
    .block(block)
    .column_spacing(2)
    .row_highlight_style(
        Style::default()
            .bg(Theme::BG_SELECTED)
            .fg(Theme::TEXT_PRIMARY),
    )
    .highlight_symbol("› ");
    let mut state = TableState::default()
        .with_selected((!rows.is_empty()).then_some(selected.saturating_sub(start)));
    f.render_stateful_widget(table, cols[0], &mut state);
    scrollbar(f, cols[0], rows.len(), visible, start);
    if cols.len() > 1 {
        body(f, cols[1], "Details", details);
    }
}
fn profiles(f: &mut Frame, app: &App, area: Rect) {
    let w = &app.snapshot.workspace;
    let entries = w.core_profiles();
    if entries.is_empty() {
        body(
            f,
            area,
            "Profiles",
            vec![
                line("Core", &w.active_core),
                Line::from(""),
                Line::from("F: enter a YAML / JSON / WireGuard file path."),
                Line::from("N: Windows file picker. G: generate from endpoint + routing."),
                Line::from("Choose a core on tab 7. I installs it when needed."),
                Line::from("Selection never starts a core or rewrites the original.")
                    .fg(Theme::TEXT_MUTED),
            ],
        );
        return;
    }
    let selected = app.selected[1].min(entries.len() - 1);
    let p = entries[selected];
    let running = app
        .snapshot
        .core_states
        .get(&w.active_core)
        .and_then(|s| s.running_config.as_ref())
        .map_or_else(|| "Not managed here".into(), |p| p.display().to_string());
    library(
        f,
        app,
        area,
        "Native profiles",
        entries
            .iter()
            .map(|p| {
                vec![
                    p.name.clone(),
                    if w.selected_profile().is_some_and(|s| s.id == p.id) {
                        "Selected".into()
                    } else if !app.preview && !p.path.is_file() {
                        "Missing".into()
                    } else {
                        p.validation.clone()
                    },
                ]
            })
            .collect(),
        vec!["Profile", "State"],
        vec![
            line("File", p.path.display().to_string()),
            line("Native check", &p.validation),
            line("Port", p.port.to_string()),
            Line::from(""),
            line("Running", running),
            Line::from(""),
            Line::from("Enter selects. V validates. C makes an editable copy."),
            Line::from("G generates a new file. S/X starts/stops the owned core."),
            Line::from("R renames. Delete removes the entry; file stays.").fg(Theme::TEXT_MUTED),
        ],
    );
}
fn cores(f: &mut Frame, app: &App, area: Rect) {
    let selected = app.selected[6];
    let details = app
        .snapshot
        .cores
        .get(selected)
        .map(|c| {
            vec![
                line("Author", &c.author),
                line("License", &c.license),
                line("Source", &c.source),
                line("Format", &c.format),
                line("Generated TUN", c.tun.to_string()),
                line("Process rules", c.process_rules.to_string()),
                Line::from(""),
                Line::from("Enter selects. E edits options. I installs."),
                Line::from("A imports an adapter. B installs a trusted local binary."),
            ]
        })
        .unwrap_or_default();
    library(
        f,
        app,
        area,
        "Core adapters",
        app.snapshot
            .cores
            .iter()
            .map(|c| {
                vec![
                    c.name.clone(),
                    if c.id == app.snapshot.workspace.active_core {
                        "Selected".into()
                    } else {
                        app.snapshot
                            .core_states
                            .get(&c.id)
                            .map_or("Unavailable", |s| s.service.label())
                            .into()
                    },
                ]
            })
            .collect(),
        vec!["Core", "State"],
        details,
    );
}
fn endpoints(f: &mut Frame, app: &App, area: Rect) {
    let w = &app.snapshot.workspace;
    if w.endpoints.is_empty() {
        body(
            f,
            area,
            "Private endpoint library",
            vec![
                Line::from("A: paste SOCKS5 / HTTP(S) / VLESS / Trojan link."),
                Line::from("W: import a single-peer WireGuard .conf."),
                Line::from(""),
                Line::from("Credentials are stored privately and hidden here."),
                Line::from("Selecting an endpoint only affects future generated profiles."),
            ],
        );
        return;
    }
    let details = w
        .endpoints
        .get(app.selected[7])
        .map(|e| {
            vec![
                line("Name", &e.name),
                line("Type", &e.kind),
                line("Server", format!("{}:{}", e.server, e.port)),
                Line::from(""),
                Line::from("Credentials hidden; originals remain untouched."),
                Line::from("Enter selects. Delete removes from library."),
                Line::from("Profiles already generated keep their original endpoint."),
            ]
        })
        .unwrap_or_default();
    library(
        f,
        app,
        area,
        "Endpoints",
        w.endpoints
            .iter()
            .map(|e| {
                vec![
                    e.name.clone(),
                    if w.active_endpoint.as_ref() == Some(&e.id) {
                        "Selected".into()
                    } else {
                        e.kind.clone()
                    },
                ]
            })
            .collect(),
        vec!["Endpoint", "Type / state"],
        details,
    );
}
fn routing(f: &mut Frame, app: &App, area: Rect) {
    let w = &app.snapshot.workspace;
    let mut rows: Vec<_> = w
        .standards
        .iter()
        .map(|s| {
            vec![
                s.name.clone(),
                if s.id == w.active_standard {
                    "Selected".into()
                } else {
                    s.fallback.clone()
                },
            ]
        })
        .collect();
    let mut details = vec![
        Line::from("A creates a standard. Enter selects it."),
        Line::from("E adds domain / process / IP rules. U/J reorders."),
        Line::from("Delete removes. T sets fallback. B edits browser split."),
        Line::from("G generates. O opens the browser PAC."),
    ];
    if let Some(s) = w.standard() {
        rows.extend(
            s.rules
                .iter()
                .map(|r| vec![format!("{}: {}", r.kind, r.value), r.action.clone()]),
        );
        details.extend([
            Line::from(""),
            line("Fallback", &s.fallback),
            line("Browser proxy domains", s.browser_domains.join(", ")),
            Line::from(""),
            Line::from("Rules run top to bottom after local network bypass."),
            Line::from("Browser PAC selects domains before the core's rules."),
            Line::from("Changes apply only to a newly generated profile."),
        ]);
    }
    library(
        f,
        app,
        area,
        "Standards & ordered rules",
        rows,
        vec!["Standard / rule", "Route"],
        details,
    );
}
fn settings(f: &mut Frame, app: &App, area: Rect) {
    let w = &app.snapshot.workspace;
    let mut lines = vec![
        line("Runtime data", app.paths.root.display().to_string()),
        Line::from("H: enter data folder. K: folder picker. O: open folder."),
        Line::from("B: backup. R: restore (cores stopped). A: toggle core autostart."),
        Line::from(""),
        line("Selected core", &w.active_core),
    ];
    if let Some(o) = w.core_options.get(&w.active_core) {
        lines.extend([
            line("DNS", o.dns.join(", ")),
            line("Strategy", &o.dns_strategy),
            line(
                "Port / IPv6 / TUN",
                format!("{} / {} / {}", o.port, o.ipv6, o.tun),
            ),
            line("Logging", &o.log_level),
        ]);
    }
    lines.extend([
        Line::from("E: edit options. G on Profiles generates a new config."),
        Line::from(""),
        Line::from("Authors: MetaCubeX / Dreamacro; nekohasekai / SagerNet; XTLS / ProjectX."),
        Line::from("Flowseal; bolvan; WireGuard; basil00; Rust, Ratatui and Crossterm."),
        Line::from("Licenses and project links: THIRD_PARTY_NOTICES.md.").fg(Theme::TEXT_MUTED),
    ]);
    body(f, area, "Storage, client DNS & credits", lines);
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
        "Selected core inbound"
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
            Line::from("URTW does not replace the selected strategy or delete services."),
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
    let sections = Layout::vertical([Constraint::Length(7), Constraint::Min(0)]).split(inner);
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
        Tab::Overview => "F/N Config · S/X Core · P Windows proxy · D Test · Q Quit",
        Tab::Profiles => "↑↓ Enter Select · F/N File · G Generate · V Check · C Copy · S/X · Q",
        Tab::Diagnostics => "↑↓ Scroll · D Test · M Change path · Tab Tabs · Q Quit",
        Tab::Telegram => "S/X Run/Stop · L Link · G Connect · E Port · O Config · I Install · Q",
        Tab::Zapret => "Enter Service manager · I Install · Tab Tabs · Q Quit",
        Tab::Components => "↑↓ Select · I/Enter Install pinned · Tab Tabs · Q Quit",
        Tab::Cores => "↑↓ Enter Select · E Options · I Install · A Adapter · S/X · Q",
        Tab::Endpoints => "↑↓ Enter Select · A Link · W WireGuard · Delete Remove · Q",
        Tab::Routing => "A Standard · E Rule · B Browser · T Default · U/J Move · G Generate · Q",
        Tab::Settings => "E Core options · H/K Folder · O Open · B Backup · R Restore · Q",
    };

    let compact = match app.tab {
        Tab::Overview => "F/N File · S/X Core · P Proxy · D Test · Q",
        Tab::Profiles => "Enter Select · F/N File · G New · V Check · C Copy · Q",
        Tab::Diagnostics => "D Test · M Path · ↑↓ Scroll · Q",
        Tab::Telegram => "S/X Run · L Link · G TG · E Port · O Edit · I · Q",
        Tab::Zapret => "Enter Manager · I Install · Q",
        Tab::Components => "↑↓ Select · Enter/I Install · Q",
        Tab::Cores => "Enter Select · E Options · I Install · A/B Custom · Q",
        Tab::Endpoints => "Enter Select · A Link · W WG · E Edit · Delete · Q",
        Tab::Routing => "A Standard · E Rule · B Browser · U/J Move · G New · Q",
        Tab::Settings => "E Options · H/K Folder · B Backup · R Restore · Q",
    };
    f.render_widget(
        Paragraph::new(fit(if area.width < 90 { compact } else { keys }, width))
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
                app.snapshot.workspace.profiles = (0..50)
                    .map(|i| crate::workspace::Profile {
                        id: format!("profile-{i}"),
                        name: format!("Профиль-{i}"),
                        core: "mihomo".into(),
                        path: format!("profiles/profile-{i}.yaml").into(),
                        port: 17890,
                        validation: "pending".into(),
                        generated: false,
                    })
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
                        assert!(text.contains(&format!("URTW v{}", env!("CARGO_PKG_VERSION"))));
                    }
                }
                app.form(
                    "Telegram local port",
                    serde_json::json!({"action":"SetTGPort"}),
                    vec![App::field("port", "Port", "1443", false)],
                );
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
    fn forms_never_render_endpoint_credentials() {
        let mut app = app();
        app.form(
            "Private endpoint",
            serde_json::json!({"action":"AddEndpoint"}),
            vec![App::field("link", "Link", "very-private-test-value", true)],
        );
        let text = screen(&app, 120, 30);
        assert!(!text.contains("very-private-test-value"));
        assert!(text.contains("•••"));
    }
    #[test]
    fn last_profile_is_visible_in_compact_view() {
        let mut app = app();
        app.tab = Tab::Profiles;
        app.snapshot.workspace.profiles = (0..50)
            .map(|i| crate::workspace::Profile {
                id: i.to_string(),
                name: format!("Profile-{i}"),
                core: "mihomo".into(),
                path: "config.yaml".into(),
                port: 17890,
                validation: "pending".into(),
                generated: false,
            })
            .collect();
        app.selected[1] = 49;
        assert!(screen(&app, 60, 18).contains("Profile-49"));
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
