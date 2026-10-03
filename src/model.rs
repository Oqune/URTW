use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Profiles,
    Diagnostics,
    Telegram,
    Zapret,
    Components,
    Cores,
    Endpoints,
    Routing,
    Settings,
}

impl Tab {
    pub const ALL: [Self; 10] = [
        Self::Overview,
        Self::Profiles,
        Self::Diagnostics,
        Self::Telegram,
        Self::Zapret,
        Self::Components,
        Self::Cores,
        Self::Endpoints,
        Self::Routing,
        Self::Settings,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Profiles => "Profiles",
            Self::Diagnostics => "Diagnostics",
            Self::Telegram => "Telegram",
            Self::Zapret => "Zapret",
            Self::Components => "Components",
            Self::Cores => "Cores",
            Self::Endpoints => "Endpoints",
            Self::Routing => "Routing",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    Mihomo,
    Singbox,
    Xray,
    Telegram,
    Zapret,
}
impl Component {
    pub const ALL: [Self; 5] = [
        Self::Mihomo,
        Self::Singbox,
        Self::Xray,
        Self::Telegram,
        Self::Zapret,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Mihomo => "mihomo",
            Self::Singbox => "singbox",
            Self::Xray => "xray",
            Self::Telegram => "telegram",
            Self::Zapret => "zapret",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Mihomo => "Mihomo",
            Self::Singbox => "sing-box",
            Self::Xray => "Xray",
            Self::Telegram => "TG WS Proxy",
            Self::Zapret => "Zapret",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub active_config: Option<PathBuf>,
    pub profiles: Vec<PathBuf>,
    pub mihomo_port: u16,
    pub tg_port: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            active_config: None,
            profiles: Vec::new(),
            mihomo_port: 17890,
            tg_port: 1443,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Service {
    pub installed: bool,
    pub running: bool,
    pub managed: bool,
    pub listening: bool,
    pub pid: Option<u32>,
    pub memory_mb: u64,
    pub uptime_secs: u64,
}
impl Service {
    pub fn label(&self) -> &'static str {
        if self.running && self.listening {
            if self.managed { "Running" } else { "External" }
        } else if self.running {
            "Starting / no listener"
        } else if self.listening {
            "External listener"
        } else if self.installed {
            "Stopped"
        } else {
            "Not installed"
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub mihomo: Service,
    pub telegram: Service,
    pub zapret: Service,
    pub proxy_enabled: bool,
    pub proxy_server: String,
    pub settings: Settings,
    pub error: Option<String>,
    pub installed_versions: std::collections::BTreeMap<String, String>,
    pub running_config: Option<PathBuf>,
    pub running_mihomo_port: Option<u16>,
    pub workspace: crate::workspace::Workspace,
    pub cores: Vec<crate::workspace::CoreInfo>,
    pub core_states: std::collections::BTreeMap<String, crate::workspace::CoreState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagStatus {
    Pending,
    Running,
    Http { code: u16, ms: u64 },
    Failed(String),
}
impl DiagStatus {
    pub fn finished(&self) -> bool {
        matches!(self, Self::Http { .. } | Self::Failed(_))
    }
    pub fn successful(&self) -> bool {
        matches!(
            self,
            Self::Http {
                code: 200..=399,
                ..
            }
        )
    }
    pub fn text(&self) -> String {
        match self {
            Self::Pending => "Pending".into(),
            Self::Running => "Testing...".into(),
            Self::Http { code, ms } => format!("HTTP {code} / {ms} ms"),
            Self::Failed(e) => e.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagTarget {
    pub name: &'static str,
    pub url: &'static str,
    pub status: DiagStatus,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub label: String,
    pub progress: Option<u16>,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub enum Action {
    Workspace {
        request: serde_json::Value,
        label: &'static str,
    },
    Install(Component),
    StartTelegram,
    StopTelegram,
    OpenZapret,
    CopyTelegramLink,
    OpenTelegram,
    EditTelegram,
    SetTelegramPort(u16),
    DisableProxy,
}
impl Action {
    pub fn command(&self) -> &'static str {
        match self {
            Self::Workspace { .. } => "Request",
            Self::Install(_) => "Install",
            Self::StartTelegram => "StartTG",
            Self::StopTelegram => "StopTG",
            Self::OpenZapret => "OpenZapret",
            Self::CopyTelegramLink => "CopyTGLink",
            Self::OpenTelegram => "OpenTelegram",
            Self::EditTelegram => "EditTG",
            Self::SetTelegramPort(_) => "SetTGPort",
            Self::DisableProxy => "DisableProxy",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Workspace { label, .. } => label,
            Self::Install(_) => "Installing component",
            Self::StartTelegram => "Starting TG WS Proxy",
            Self::StopTelegram => "Stopping TG WS Proxy",
            Self::OpenZapret => "Opening Zapret service manager",
            Self::CopyTelegramLink => "Copying Telegram link",
            Self::OpenTelegram => "Opening Telegram",
            Self::EditTelegram => "Opening Telegram configuration",
            Self::SetTelegramPort(_) => "Saving Telegram port",
            Self::DisableProxy => "Restoring Windows proxy",
        }
    }
}

pub fn viewport_start(selected: usize, total: usize, height: usize) -> usize {
    selected
        .saturating_add(1)
        .saturating_sub(height)
        .min(total.saturating_sub(height))
}

pub fn completed_percent(completed: usize, total: usize) -> u16 {
    if total == 0 {
        0
    } else {
        ((completed.min(total) as u128 * 100) / total as u128) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_counts_failures_as_completed() {
        let statuses = [
            DiagStatus::Http { code: 403, ms: 10 },
            DiagStatus::Failed("Timeout".into()),
            DiagStatus::Pending,
        ];
        assert_eq!(
            completed_percent(
                statuses.iter().filter(|s| s.finished()).count(),
                statuses.len()
            ),
            66
        );
        assert_eq!(completed_percent(3, 3), 100);
        assert_eq!(completed_percent(8, 3), 100);
        assert_eq!(completed_percent(0, 0), 0);
        assert!(!statuses[0].successful());
    }
    #[test]
    fn selected_row_stays_in_viewport() {
        for total in 1..100 {
            for height in 1..40 {
                for selected in 0..total {
                    let start = viewport_start(selected, total, height);
                    assert!(start <= selected && selected < start + height);
                    assert!(start <= total.saturating_sub(height));
                }
            }
        }
    }
}
