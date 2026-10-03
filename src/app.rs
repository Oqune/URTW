use crate::{
    model::*,
    network::NetworkTester,
    system::{self, Paths},
};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub enum AppEvent {
    Snapshot(Box<Snapshot>),
    Picked(Result<Option<std::path::PathBuf>, String>),
    OperationProgress {
        progress: Option<u16>,
        detail: String,
    },
    OperationFinished(Result<(), String>),
    Diagnostic {
        index: usize,
        status: DiagStatus,
    },
    DiagnosticsFinished,
}

pub struct App {
    pub paths: Paths,
    pub tab: Tab,
    pub snapshot: Snapshot,
    pub initialized: bool,
    pub preview: bool,
    pub selected: [usize; 6],
    pub diag_targets: Vec<DiagTarget>,
    pub diag_running: bool,
    pub diag_via_proxy: bool,
    pub operation: Option<Operation>,
    pub notice: Option<(String, Instant, bool)>,
    pub port_input: Option<String>,
    pub ticks: u64,
    pub tx: mpsc::Sender<AppEvent>,
}
impl App {
    pub fn new(paths: Paths, tx: mpsc::Sender<AppEvent>) -> Self {
        let targets = [
            ("GitHub", "https://github.com"),
            ("ChatGPT", "https://chatgpt.com"),
            ("Claude", "https://claude.ai"),
            ("Google AI Studio", "https://aistudio.google.com"),
            ("YouTube", "https://www.youtube.com"),
            ("Discord", "https://discord.com"),
            ("Telegram Web", "https://web.telegram.org"),
            ("Steam", "https://store.steampowered.com"),
            ("Yandex", "https://ya.ru"),
            ("Cloudflare", "https://www.cloudflare.com"),
        ];
        Self {
            paths,
            tab: Tab::Overview,
            snapshot: Snapshot::default(),
            initialized: false,
            preview: false,
            selected: [0; 6],
            diag_targets: targets
                .into_iter()
                .map(|(name, url)| DiagTarget {
                    name,
                    url,
                    status: DiagStatus::Pending,
                })
                .collect(),
            diag_running: false,
            diag_via_proxy: true,
            operation: None,
            notice: None,
            port_input: None,
            ticks: 0,
            tx,
        }
    }
    pub fn notify(&mut self, message: impl Into<String>, error: bool) {
        self.notice = Some((
            message.into(),
            Instant::now() + Duration::from_secs(if error { 12 } else { 6 }),
            error,
        ));
    }
    pub fn move_tab(&mut self, delta: isize) {
        self.tab = Tab::ALL[(self.tab.index() as isize + delta).rem_euclid(6) as usize];
    }
    pub fn move_selection(&mut self, delta: isize) {
        let len = self.list_len();
        let idx = &mut self.selected[self.tab.index()];
        *idx = (*idx as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
    }
    pub fn list_len(&self) -> usize {
        match self.tab {
            Tab::Profiles => self.snapshot.settings.profiles.len(),
            Tab::Diagnostics => self.diag_targets.len(),
            Tab::Components => Component::ALL.len(),
            _ => 0,
        }
    }
    pub fn action(&mut self, action: Action) {
        if self.operation.is_some() {
            self.notify("An operation is already running", true);
            return;
        }
        self.operation = Some(Operation {
            label: action.label().into(),
            progress: None,
            detail: "Preparing...".into(),
        });
        let paths = self.paths.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = system::run_action(&paths, &action, &tx)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::OperationFinished(result)).await;
        });
    }
    pub fn pick_config(&mut self) {
        if self.operation.is_some() {
            self.notify("Wait for the current operation", true);
            return;
        }
        self.operation = Some(Operation {
            label: "Choose a configuration file".into(),
            progress: None,
            detail: "Windows file picker is open".into(),
        });
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(system::pick_configuration)
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));
            let _ = tx.send(AppEvent::Picked(result)).await;
        });
    }
    pub fn diagnostics(&mut self) {
        if self.diag_running {
            self.notify("Diagnostics are already running", false);
            return;
        }
        let port = self
            .diag_via_proxy
            .then_some(self.snapshot.settings.mihomo_port);
        let tester = match NetworkTester::new(port) {
            Ok(t) => t,
            Err(e) => {
                self.notify(e.to_string(), true);
                return;
            }
        };
        self.diag_running = true;
        for t in &mut self.diag_targets {
            t.status = DiagStatus::Pending;
        }
        let targets = self.diag_targets.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            for (index, target) in targets.iter().enumerate() {
                let _ = tx
                    .send(AppEvent::Diagnostic {
                        index,
                        status: DiagStatus::Running,
                    })
                    .await;
                let status = tester.test(target.url).await;
                let _ = tx.send(AppEvent::Diagnostic { index, status }).await;
            }
            let _ = tx.send(AppEvent::DiagnosticsFinished).await;
        });
    }
    pub fn event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Snapshot(snapshot) => {
                self.snapshot = *snapshot;
                self.initialized = true;
                self.selected[1] =
                    self.selected[1].min(self.snapshot.settings.profiles.len().saturating_sub(1));
            }
            AppEvent::Picked(result) => {
                self.operation = None;
                match result {
                    Ok(Some(p)) => self.action(Action::SelectConfig(p)),
                    Ok(None) => self.notify("Configuration selection cancelled", false),
                    Err(e) => self.notify(e, true),
                }
            }
            AppEvent::OperationProgress { progress, detail } => {
                if let Some(op) = &mut self.operation {
                    op.progress = progress;
                    op.detail = detail;
                }
            }
            AppEvent::OperationFinished(result) => {
                self.operation = None;
                match result {
                    Ok(()) => self.notify("Operation completed; refreshing status", false),
                    Err(e) => self.notify(e, true),
                }
            }
            AppEvent::Diagnostic { index, status } => {
                if let Some(t) = self.diag_targets.get_mut(index) {
                    t.status = status;
                }
            }
            AppEvent::DiagnosticsFinished => {
                self.diag_running = false;
                self.notify(
                    "Diagnostics completed; HTTP denials and failures are listed separately",
                    false,
                );
            }
        }
    }
}
