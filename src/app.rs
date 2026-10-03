use crate::{
    model::*,
    network::NetworkTester,
    system::{self, Paths},
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
pub enum AppEvent {
    Snapshot {
        root: PathBuf,
        snapshot: Box<Snapshot>,
    },
    Picked {
        folder: bool,
        result: Result<Option<PathBuf>, String>,
    },
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
#[derive(Clone)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub secret: bool,
}
pub struct Input {
    pub title: &'static str,
    pub fields: Vec<Field>,
    pub index: usize,
    pub request: Value,
}
pub struct App {
    pub paths: Paths,
    pub tab: Tab,
    pub snapshot: Snapshot,
    pub initialized: bool,
    pub preview: bool,
    pub selected: [usize; 10],
    pub diag_targets: Vec<DiagTarget>,
    pub diag_running: bool,
    pub diag_via_proxy: bool,
    pub operation: Option<Operation>,
    pub notice: Option<(String, Instant, bool)>,
    pub input: Option<Input>,
    pub ticks: u64,
    pub tx: mpsc::Sender<AppEvent>,
    pub refreshing: bool,
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
            tx,
            tab: Tab::Overview,
            snapshot: Snapshot::default(),
            initialized: false,
            preview: false,
            selected: [0; 10],
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
            input: None,
            ticks: 0,
            refreshing: false,
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
        self.tab = Tab::ALL
            [(self.tab.index() as isize + delta).rem_euclid(Tab::ALL.len() as isize) as usize];
    }
    pub fn move_selection(&mut self, delta: isize) {
        let len = self.list_len();
        let i = &mut self.selected[self.tab.index()];
        *i = (*i as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
    }
    pub fn list_len(&self) -> usize {
        match self.tab {
            Tab::Profiles => self.snapshot.workspace.core_profiles().len(),
            Tab::Diagnostics => self.diag_targets.len(),
            Tab::Components => Component::ALL.len(),
            Tab::Cores => self.snapshot.cores.len(),
            Tab::Endpoints => self.snapshot.workspace.endpoints.len(),
            Tab::Routing => {
                self.snapshot.workspace.standards.len()
                    + self
                        .snapshot
                        .workspace
                        .standard()
                        .map_or(0, |s| s.rules.len())
            }
            _ => 0,
        }
    }
    pub fn request(&mut self, request: Value, label: &'static str) {
        self.action(Action::Workspace { request, label });
    }
    pub fn core_request(&mut self, action: &str, label: &'static str) {
        self.request(
            json!({"action":action,"core":self.snapshot.workspace.active_core}),
            label,
        );
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
    pub fn refresh(&mut self) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let paths = self.paths.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let root = paths.root.clone();
            if let Ok(s) = tokio::task::spawn_blocking(move || system::inspect(&paths)).await {
                let _ = tx
                    .send(AppEvent::Snapshot {
                        root,
                        snapshot: Box::new(s),
                    })
                    .await;
            }
        });
    }
    pub fn pick(&mut self, folder: bool) {
        if self.operation.is_some() {
            self.notify("Wait for the current operation", true);
            return;
        }
        self.operation = Some(Operation {
            label: "Choose a file or data folder".into(),
            progress: None,
            detail: "Windows picker is open; Alt+Tab if it is behind the terminal".into(),
        });
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                if folder {
                    system::pick_folder()
                } else {
                    system::pick_configuration()
                }
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));
            let _ = tx.send(AppEvent::Picked { folder, result }).await;
        });
    }
    pub fn change_root(&mut self, path: PathBuf) {
        match Paths::discover(Some(path), Some(self.paths.assets.clone())).and_then(|p| {
            if p.root == p.assets || p.root.parent().is_none() {
                anyhow::bail!("Choose a dedicated data folder")
            };
            p.remember()?;
            Ok(p)
        }) {
            Ok(p) => {
                self.paths = p;
                self.snapshot = Snapshot::default();
                self.initialized = false;
                self.selected = [0; 10];
                self.refreshing = false;
                self.refresh();
                self.notify("Data folder selected. Existing processes keep running in their original runtime.",false)
            }
            Err(e) => self.notify(e.to_string(), true),
        }
    }
    pub fn form(&mut self, title: &'static str, request: Value, fields: Vec<Field>) {
        if self.operation.is_none() {
            self.input = Some(Input {
                title,
                request,
                fields,
                index: 0,
            });
        }
    }
    pub fn field(
        key: &'static str,
        label: &'static str,
        value: impl Into<String>,
        secret: bool,
    ) -> Field {
        Field {
            key,
            label,
            value: value.into(),
            secret,
        }
    }
    pub fn submit(&mut self) {
        let Some(mut form) = self.input.take() else {
            return;
        };
        for field in form.fields {
            form.request[field.key] = json!(field.value.trim());
        }
        let action = form.request["action"].as_str().unwrap_or("").to_owned();
        if action == "Storage" {
            self.change_root(PathBuf::from(form.request["path"].as_str().unwrap_or("")));
            return;
        }
        if action == "CoreOptions" {
            let parsed = core_options_request(&form.request);
            match parsed {
                Ok(r) => form.request = r,
                Err(e) => {
                    self.notify(e, true);
                    return;
                }
            }
        }
        if action == "SetTGPort" {
            match form.request["port"]
                .as_str()
                .and_then(|s| s.parse::<u16>().ok())
                .filter(|p| *p >= 1024)
            {
                Some(port) => self.action(Action::SetTelegramPort(port)),
                None => self.notify("Enter a port from 1024 to 65535", true),
            };
            return;
        }
        self.request(form.request, form.title);
    }
    pub fn diagnostics(&mut self) {
        if self.diag_running {
            self.notify("Diagnostics are already running", false);
            return;
        }
        let id = &self.snapshot.workspace.active_core;
        let port = self.diag_via_proxy.then_some(
            self.snapshot
                .core_states
                .get(id)
                .and_then(|s| s.running_port)
                .or_else(|| self.snapshot.workspace.selected_profile().map(|p| p.port))
                .or_else(|| self.snapshot.workspace.core_options.get(id).map(|o| o.port))
                .unwrap_or(17890),
        );
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
            AppEvent::Snapshot { root, snapshot } => {
                self.refreshing = false;
                if root != self.paths.root {
                    return;
                }
                self.snapshot = *snapshot;
                self.initialized = true;
                for tab in Tab::ALL {
                    let old = self.tab;
                    self.tab = tab;
                    self.selected[tab.index()] =
                        self.selected[tab.index()].min(self.list_len().saturating_sub(1));
                    self.tab = old;
                }
            }
            AppEvent::Picked { folder, result } => {
                self.operation = None;
                match result {
                    Ok(Some(path)) => {
                        if folder {
                            self.change_root(path)
                        } else {
                            self.request(json!({"action":"SelectFile","core":self.snapshot.workspace.active_core,"path":path}),"Selecting configuration")
                        }
                    }
                    Ok(None) => self.notify("Selection cancelled", false),
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
                    Ok(()) => self.notify("Completed; refreshing status", false),
                    Err(e) => self.notify(e, true),
                }
                self.refresh();
            }
            AppEvent::Diagnostic { index, status } => {
                if let Some(t) = self.diag_targets.get_mut(index) {
                    t.status = status;
                }
            }
            AppEvent::DiagnosticsFinished => {
                self.diag_running = false;
                self.notify(
                    "Completed; HTTP denials and transport failures are listed separately",
                    false,
                );
            }
        }
    }
}
fn core_options_request(request: &Value) -> Result<Value, String> {
    let text = |key: &str| request[key].as_str().unwrap_or("");
    let port = text("port")
        .parse::<u16>()
        .ok()
        .filter(|p| *p >= 1024)
        .ok_or("Port must be 1024..65535")?;
    let bool_value = |key: &str| match text(key) {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{key} must be true or false")),
    };
    Ok(
        json!({"action":"CoreOptions","core":request["core"],"options":{"port":port,"dns":text("dns").split(',').map(str::trim).filter(|s|!s.is_empty()).collect::<Vec<_>>(),"dns_strategy":text("dns_strategy"),"ipv6":bool_value("ipv6")?,"tun":bool_value("tun")?,"log_level":text("log_level")}}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_navigation_tabs_are_reachable() {
        let (tx, _) = mpsc::channel(1);
        let mut a = App::new(
            Paths {
                root: "runtime".into(),
                assets: "assets".into(),
            },
            tx,
        );
        a.move_tab(-1);
        assert_eq!(a.tab, Tab::Settings);
        a.move_tab(1);
        assert_eq!(a.tab, Tab::Overview);
    }
    #[test]
    fn options_form_rejects_invalid_values() {
        let r = json!({"port":"0"});
        assert!(core_options_request(&r).is_err());
        let r = json!({"port":"17890","dns":"1.1.1.1, https://example.com/dns-query","ipv6":"false","tun":"false","dns_strategy":"ipv4_only","log_level":"warning"});
        assert_eq!(
            core_options_request(&r).unwrap()["options"]["dns"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn stale_snapshot_cannot_change_new_storage() {
        let (tx, _) = mpsc::channel(1);
        let mut a = App::new(
            Paths {
                root: "new".into(),
                assets: "assets".into(),
            },
            tx,
        );
        a.event(AppEvent::Snapshot {
            root: "old".into(),
            snapshot: Box::default(),
        });
        assert!(!a.initialized);
    }
}
