use crate::{app::AppEvent, model::*};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    fs,
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use sysinfo::{Pid, System};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    sync::mpsc,
};

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub assets: PathBuf,
}
impl Paths {
    pub fn discover(root: Option<PathBuf>, assets: Option<PathBuf>) -> Result<Self> {
        let root = root
            .or_else(|| std::env::var_os("URT_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("URT")))
            .context("Set --root or URT_HOME when LOCALAPPDATA is unavailable")?;
        let assets = if let Some(p) = assets {
            p
        } else {
            let exe = std::env::current_exe()?;
            exe.parent()
                .context("Executable directory unavailable")?
                .ancestors()
                .find(|p| p.join("routing.ps1").is_file())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| exe.parent().unwrap().to_path_buf())
        };
        if !root.is_absolute() || !assets.is_absolute() {
            bail!("Runtime and assets paths must be absolute");
        }
        Ok(Self { root, assets })
    }
    pub fn pinned_version(&self, component: Component) -> String {
        fs::read(self.assets.join("components.lock.json"))
            .ok()
            .and_then(|v| serde_json::from_slice::<serde_json::Value>(&v).ok())
            .and_then(|v| v[component.id()]["version"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "Unavailable".into())
    }
}

#[derive(Deserialize)]
struct ProcessRecord {
    pid: u32,
    path: PathBuf,
    start_time: u64,
    #[serde(default)]
    config: Option<PathBuf>,
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .eq_ignore_ascii_case(&b.to_string_lossy())
}
fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(60),
    )
    .is_ok()
}

pub fn inspect(paths: &Paths) -> Snapshot {
    let mut snapshot = Snapshot::default();
    let settings_path = paths.root.join("settings.json");
    if settings_path.exists() {
        match fs::read(settings_path)
            .map_err(|_| "Cannot read runtime settings")
            .and_then(|b| {
                serde_json::from_slice::<Settings>(&b)
                    .map_err(|_| "Invalid settings.json; restore a valid runtime settings file")
            }) {
            Ok(s) if s.schema_version == 1 => snapshot.settings = s,
            Ok(_) => snapshot.error = Some("Unsupported settings schema".into()),
            Err(e) => snapshot.error = Some(e.into()),
        }
    }
    snapshot.installed_versions = fs::read(paths.root.join("installed.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let sys = System::new_all();
    for (component, port) in [
        (Component::Mihomo, snapshot.settings.mihomo_port),
        (Component::Telegram, snapshot.settings.tg_port),
        (Component::Zapret, 0),
    ] {
        let record: Option<ProcessRecord> =
            fs::read(paths.root.join(format!("{}.pid.json", component.id())))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
        let owned = record
            .as_ref()
            .filter(|r| {
                let expected = match component {
                    Component::Mihomo => paths.root.join("tools/mihomo/mihomo.exe"),
                    Component::Telegram => paths.root.join("tools/telegram/TgWsProxy_windows.exe"),
                    Component::Zapret => paths.root.join("tools/zapret/bin/winws.exe"),
                };
                same_path(&r.path, &expected)
            })
            .and_then(|r| sys.process(Pid::from_u32(r.pid)))
            .filter(|p| {
                record.as_ref().is_some_and(|r| {
                    p.exe().is_some_and(|e| same_path(e, &r.path)) && p.start_time() == r.start_time
                })
            });
        let external = sys.processes().values().find(|p| {
            let name = p.name().to_string_lossy().to_lowercase();
            match component {
                Component::Mihomo => name.contains("mihomo"),
                Component::Telegram => name.contains("tgwsproxy"),
                Component::Zapret => name == "winws.exe" || name == "winws",
            }
        });
        let process = owned.or(external);
        let installed = match component {
            Component::Mihomo => paths.root.join("tools/mihomo/mihomo.exe").is_file(),
            Component::Telegram => paths
                .root
                .join("tools/telegram/TgWsProxy_windows.exe")
                .is_file(),
            Component::Zapret => paths.root.join("tools/zapret/service.bat").is_file(),
        };
        let state = Service {
            installed,
            running: process.is_some(),
            managed: owned.is_some(),
            listening: if port == 0 {
                process.is_some()
            } else {
                port_open(port)
            },
            pid: process.map(|p| p.pid().as_u32()),
            memory_mb: process.map_or(0, |p| p.memory() / (1024 * 1024)),
            uptime_secs: process.map_or(0, |p| p.run_time()),
        };
        match component {
            Component::Mihomo => {
                snapshot.mihomo = state;
                if owned.is_some() {
                    snapshot.running_config = record.and_then(|r| r.config);
                }
            }
            Component::Telegram => snapshot.telegram = state,
            Component::Zapret => snapshot.zapret = state,
        }
    }
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        {
            snapshot.proxy_enabled = key.get_value::<u32, _>("ProxyEnable").unwrap_or(0) == 1;
            snapshot.proxy_server = key
                .get_value::<String, _>("ProxyServer")
                .unwrap_or_default();
        }
    }
    snapshot
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

pub fn pick_configuration() -> Result<Option<PathBuf>> {
    if !cfg!(windows) {
        bail!("The native configuration picker requires Windows");
    }
    let mut cmd = Command::new("powershell.exe");
    hidden(&mut cmd);
    let output = cmd.args(["-NoProfile", "-STA", "-Command", "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); Add-Type -AssemblyName System.Windows.Forms; $d=[Windows.Forms.OpenFileDialog]::new(); $d.Title='Select a Mihomo YAML or WireGuard profile'; $d.Filter='Routing configurations (*.yaml;*.yml;*.conf)|*.yaml;*.yml;*.conf'; $d.CheckFileExists=$true; if($d.ShowDialog() -eq 'OK'){[Console]::Write($d.FileName)}; $d.Dispose()"]).output()?;
    if !output.status.success() {
        bail!("Could not open the Windows file picker");
    }
    let path = String::from_utf8(output.stdout)?.trim().to_owned();
    Ok(if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    })
}

pub async fn run_action(paths: &Paths, action: &Action, tx: &mpsc::Sender<AppEvent>) -> Result<()> {
    let script = paths.assets.join("routing.ps1");
    if !script.is_file() {
        bail!("routing.ps1 is missing beside the application assets");
    }
    let mut cmd = tokio::process::Command::new("powershell.exe");
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    cmd.args(["-NoProfile", "-STA", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .arg("-Root")
        .arg(&paths.root)
        .arg("-Command")
        .arg(action.command());
    match action {
        Action::SelectConfig(path) => {
            cmd.arg("-Path").arg(path);
        }
        Action::Install(c) => {
            cmd.arg("-Component").arg(c.id());
        }
        Action::SetTelegramPort(port) => {
            cmd.arg("-Port").arg(port.to_string());
        }
        _ => {}
    }
    let mut child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Could not start the Windows backend")?;
    let stderr = child
        .stderr
        .take()
        .context("Missing backend error stream")?;
    let error_task = tokio::spawn(async move {
        let mut data = Vec::new();
        let _ = stderr.take(16_384).read_to_end(&mut data).await;
        String::from_utf8_lossy(&data).into_owned()
    });
    let mut lines = BufReader::new(
        child
            .stdout
            .take()
            .context("Missing backend output stream")?,
    )
    .lines();
    while let Some(line) = lines.next_line().await? {
        if let Some(s) = line.strip_prefix("URT_PROGRESS|") {
            let mut parts = s.splitn(2, '|');
            let progress = parts
                .next()
                .and_then(|v| v.parse::<u16>().ok())
                .map(|p| p.min(100));
            let detail = parts
                .next()
                .unwrap_or("")
                .chars()
                .filter(|c| !c.is_control())
                .collect();
            let _ = tx
                .send(AppEvent::OperationProgress { progress, detail })
                .await;
        }
    }
    let status = child.wait().await?;
    let errors = error_task.await?;
    if !status.success() {
        let detail = errors
            .lines()
            .find_map(|s| s.strip_prefix("URT_ERROR|"))
            .unwrap_or("Operation failed; inspect the component log in the runtime folder");
        bail!("{}", detail.chars().take(240).collect::<String>());
    }
    Ok(())
}
