use crate::{
    app::AppEvent,
    model::*,
    workspace::{CoreInfo, CoreState, Workspace},
};
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
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub assets: PathBuf,
}
impl Paths {
    pub fn discover(root: Option<PathBuf>, assets: Option<PathBuf>) -> Result<Self> {
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
        let remembered = fs::read(assets.join("urt-location.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v["root"].as_str().map(PathBuf::from));
        let root = root
            .or_else(|| std::env::var_os("URTW_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("URT_HOME").map(PathBuf::from))
            .or(remembered)
            .or_else(|| {
                assets
                    .join("portable.flag")
                    .is_file()
                    .then(|| assets.join("data"))
            })
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("URTW")))
            .context("Set --root or URTW_HOME")?;
        if !root.is_absolute() || !assets.is_absolute() {
            bail!("Runtime and assets paths must be absolute")
        }
        Ok(Self {
            root: std::path::absolute(root)?,
            assets: std::path::absolute(assets)?,
        })
    }
    pub fn pinned_version(&self, component: Component) -> String {
        fs::read(self.assets.join("components.lock.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v[component.id()]["version"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "Unavailable".into())
    }
    pub fn remember(&self) -> Result<()> {
        let file = self.assets.join("urt-location.json");
        let tmp = file.with_extension("json.tmp");
        fs::write(
            &tmp,
            serde_json::to_vec(&serde_json::json!({"root":self.root}))?,
        )?;
        if file.exists() {
            fs::remove_file(&file)?;
        }
        fs::rename(tmp, file)?;
        Ok(())
    }
}
#[derive(Deserialize)]
struct ProcessRecord {
    pid: u32,
    path: PathBuf,
    start_time: u64,
    #[serde(default)]
    config: Option<PathBuf>,
    #[serde(default)]
    port: u16,
}
fn same_path(a: &Path, b: &Path) -> bool {
    let a = std::path::absolute(a).unwrap_or_else(|_| a.into());
    let b = std::path::absolute(b).unwrap_or_else(|_| b.into());
    a.components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .eq(b
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase()))
}
fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(60),
    )
    .is_ok()
}
pub fn inspect(paths: &Paths) -> Snapshot {
    let mut s = Snapshot::default();
    if let Ok(bytes) = fs::read(paths.root.join("settings.json")) {
        match serde_json::from_slice::<Settings>(&bytes) {
            Ok(v) if v.schema_version == 1 => s.settings = v,
            _ => s.error = Some("Invalid settings.json; restore a private backup".into()),
        }
    }
    s.installed_versions = fs::read(paths.root.join("installed.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let workspace_file = paths.root.join("workspace.json");
    match fs::read(&workspace_file) {
        Ok(b) => match serde_json::from_slice::<Workspace>(&b) {
            Ok(w) if w.schema_version == 1 => s.workspace = w,
            _ => s.error = Some("Invalid workspace.json; restore a private backup".into()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            s.workspace = fs::read(paths.assets.join("config/workspace-defaults.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default()
        }
        Err(_) => s.error = Some("Cannot read workspace".into()),
    }
    s.cores = fs::read(paths.assets.join("config/cores.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| serde_json::from_value(v["cores"].clone()).ok())
        .unwrap_or_default();
    if let Ok(entries) = fs::read_dir(paths.root.join("core-adapters")) {
        for entry in entries.flatten() {
            if entry.path().extension().is_some_and(|e| e == "json")
                && let Ok(b) = fs::read(entry.path())
                && let Ok(core) = serde_json::from_slice::<CoreInfo>(&b)
                && !s.cores.iter().any(|c| c.id == core.id)
            {
                s.cores.push(core);
            }
        }
    }
    // Read-only legacy migration view. The backend persists migration on an explicit action.
    if !workspace_file.exists() {
        for (i, path) in s.settings.profiles.iter().enumerate() {
            let id = format!("legacy-{i}");
            s.workspace.profiles.push(crate::workspace::Profile {
                id: id.clone(),
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                core: "mihomo".into(),
                path: path.clone(),
                port: s.settings.mihomo_port,
                validation: "pending".into(),
                generated: false,
            });
            if s.settings.active_config.as_ref() == Some(path) {
                s.workspace.selected_profiles.insert("mihomo".into(), id);
            }
        }
    }
    let aliases: serde_json::Value = fs::read(paths.root.join("installed-paths.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let sys = System::new_all();
    let mut items: Vec<(String, PathBuf, u16)> = s
        .cores
        .iter()
        .map(|c| {
            (
                c.id.clone(),
                paths.root.join("tools").join(&c.id).join(&c.executable),
                s.workspace
                    .core_options
                    .get(&c.id)
                    .map_or(c.default_port, |o| o.port),
            )
        })
        .collect();
    items.extend([
        (
            "telegram".into(),
            paths.root.join("tools/telegram/TgWsProxy_windows.exe"),
            s.settings.tg_port,
        ),
        (
            "zapret".into(),
            paths.root.join("tools/zapret/bin/winws.exe"),
            0,
        ),
    ]);
    for (id, default_exe, port) in items {
        let expected = aliases[&id]
            .as_str()
            .map(PathBuf::from)
            .filter(|p| {
                std::path::absolute(p)
                    .ok()
                    .is_some_and(|p| p.starts_with(&paths.root))
            })
            .unwrap_or(default_exe);
        let record: Option<ProcessRecord> = fs::read(paths.root.join(format!("{id}.pid.json")))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let owned = record
            .as_ref()
            .filter(|r| same_path(&r.path, &expected))
            .and_then(|r| sys.process(Pid::from_u32(r.pid)))
            .filter(|p| {
                record.as_ref().is_some_and(|r| {
                    p.exe().is_some_and(|e| same_path(e, &r.path)) && p.start_time() == r.start_time
                })
            });
        let external = sys.processes().values().find(|p| {
            let n = p.name().to_string_lossy().to_lowercase();
            match id.as_str() {
                "mihomo" => n.contains("mihomo"),
                "telegram" => n.contains("tgwsproxy"),
                "zapret" => n == "winws.exe" || n == "winws",
                _ => expected
                    .file_name()
                    .is_some_and(|v| n == v.to_string_lossy().to_lowercase()),
            }
        });
        let process = owned.or(external);
        let listen_port = owned
            .and_then(|_| record.as_ref().map(|r| r.port))
            .filter(|p| *p > 0)
            .unwrap_or(port);
        let state = Service {
            installed: if id == "zapret" {
                paths.root.join("tools/zapret/service.bat").is_file()
            } else {
                expected.is_file()
            },
            running: process.is_some(),
            managed: owned.is_some(),
            listening: if port == 0 {
                process.is_some()
            } else {
                port_open(listen_port)
            },
            pid: process.map(|p| p.pid().as_u32()),
            memory_mb: process.map_or(0, |p| p.memory() / (1024 * 1024)),
            uptime_secs: process.map_or(0, |p| p.run_time()),
        };
        let config = owned.and_then(|_| record.as_ref().and_then(|r| r.config.clone()));
        match id.as_str() {
            "mihomo" => {
                s.mihomo = state.clone();
                s.running_config = config.clone();
                s.running_mihomo_port = owned.map(|_| listen_port)
            }
            "telegram" => s.telegram = state.clone(),
            "zapret" => s.zapret = state.clone(),
            _ => {}
        }
        s.core_states.insert(
            id,
            CoreState {
                service: state,
                running_config: config,
                running_port: owned.map(|_| listen_port),
            },
        );
    }
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        {
            s.proxy_enabled = key.get_value::<u32, _>("ProxyEnable").unwrap_or(0) == 1;
            s.proxy_server = key
                .get_value::<String, _>("ProxyServer")
                .unwrap_or_default();
        }
    }
    s
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
    pick(false)
}
pub fn pick_folder() -> Result<Option<PathBuf>> {
    pick(true)
}
fn pick(folder: bool) -> Result<Option<PathBuf>> {
    if !cfg!(windows) {
        bail!("The native picker requires Windows")
    }
    let script = if folder {
        "$d=[Windows.Forms.FolderBrowserDialog]::new();$d.Description='Choose a private URTW runtime folder';if($d.ShowDialog() -eq 'OK'){[Console]::Write($d.SelectedPath)};$d.Dispose()"
    } else {
        "$d=[Windows.Forms.OpenFileDialog]::new();$d.Title='Choose a native YAML / JSON or WireGuard profile';$d.Filter='Routing profiles|*.yaml;*.yml;*.json;*.conf|All files|*.*';$d.CheckFileExists=$true;if($d.ShowDialog() -eq 'OK'){[Console]::Write($d.FileName)};$d.Dispose()"
    };
    let mut cmd = Command::new("powershell.exe");
    hidden(&mut cmd);
    let output=cmd.args(["-NoProfile","-STA","-Command",&format!("[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);Add-Type -AssemblyName System.Windows.Forms;{script}")]).output()?;
    if !output.status.success() {
        bail!("Could not open the Windows picker; enter the path with F")
    }
    let path = String::from_utf8(output.stdout)?.trim().to_owned();
    Ok((!path.is_empty()).then(|| path.into()))
}
pub async fn run_action(paths: &Paths, action: &Action, tx: &mpsc::Sender<AppEvent>) -> Result<()> {
    let script = paths.assets.join("routing.ps1");
    if !script.is_file() {
        bail!("routing.ps1 is missing beside the application assets")
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
        Action::Install(c) => {
            cmd.arg("-Component").arg(c.id());
        }
        Action::SetTelegramPort(port) => {
            cmd.arg("-Port").arg(port.to_string());
        }
        _ => {}
    }
    let mut child = cmd
        .stdin(if matches!(action, Action::Workspace { .. }) {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Could not start the Windows backend")?;
    if let Action::Workspace { request, .. } = action {
        let mut input = child.stdin.take().context("Missing request stream")?;
        input.write_all(&serde_json::to_vec(request)?).await?;
        input.shutdown().await?;
    }
    let stderr = child
        .stderr
        .take()
        .context("Missing backend error stream")?;
    let error_task = tokio::spawn(async move {
        let mut data = Vec::new();
        let _ = stderr.take(16384).read_to_end(&mut data).await;
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
            .unwrap_or("Operation failed; inspect the private component log");
        bail!("{}", detail.chars().take(240).collect::<String>());
    }
    Ok(())
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn ownership_paths_accept_windows_separators_and_case() {
        assert!(same_path(
            Path::new(r"C:\URTW-data\tools/mihomo/mihomo.exe"),
            Path::new(r"c:\urtw-data\tools\mihomo\mihomo.exe")
        ));
        assert!(!same_path(
            Path::new(r"C:\URTW-data\tools\mihomo\mihomo.exe"),
            Path::new(r"C:\URTW-other\tools\mihomo\mihomo.exe")
        ));
    }
    #[test]
    fn explicit_roots_are_normalized_without_creating_directories() {
        let p = Paths::discover(
            Some(r"C:\URTW-preview\unused\..\data".into()),
            Some(r"C:\URTW-preview\assets".into()),
        )
        .unwrap();
        assert!(same_path(&p.root, Path::new(r"C:\URTW-preview\data")));
    }
}
