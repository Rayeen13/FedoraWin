use serde::{Deserialize, Serialize};
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub name: String,
    pub app_id: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StartAppRaw {
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "AppID")]
    app_id: Option<String>,
}

fn push_aliases(aliases: &mut Vec<String>, values: &[&str]) {
    aliases.extend(values.iter().copied().map(str::to_string));
}

fn aliases_for(name: &str, app_id: &str) -> Vec<String> {
    let n = name.to_ascii_lowercase();
    let id = app_id.to_ascii_lowercase();
    let mut aliases = Vec::new();

    if n.contains("windows terminal") || id.contains("windowsterminal") {
        push_aliases(
            &mut aliases,
            &[
                "terminal",
                "term",
                "console",
                "shell",
                "command line",
                "cli",
            ],
        );
    }
    if n.contains("powershell") {
        push_aliases(
            &mut aliases,
            &[
                "terminal",
                "shell",
                "pwsh",
                "powershell",
                "command line",
                "cli",
            ],
        );
    }
    if n.contains("command prompt") || n == "cmd" || id.ends_with("cmd.exe") {
        push_aliases(
            &mut aliases,
            &[
                "terminal",
                "console",
                "cmd",
                "command prompt",
                "command line",
                "cli",
            ],
        );
    }
    if n.contains("file explorer") || n == "explorer" || id.contains("explorer.exe") {
        push_aliases(
            &mut aliases,
            &[
                "files",
                "file manager",
                "explorer",
                "folders",
                "home folder",
            ],
        );
    }
    if n.contains("settings") || id.contains("immersivecontrolpanel") {
        push_aliases(
            &mut aliases,
            &[
                "settings",
                "preferences",
                "control panel",
                "system settings",
            ],
        );
    }
    if ["edge", "chrome", "firefox", "opera", "brave", "vivaldi"]
        .iter()
        .any(|browser| n.contains(browser))
    {
        push_aliases(&mut aliases, &["browser", "web", "internet", "web browser"]);
    }
    if n.contains("notepad") || n.contains("text editor") {
        push_aliases(&mut aliases, &["text editor", "editor", "notes", "notepad"]);
    }
    if n.contains("calculator") {
        push_aliases(&mut aliases, &["calculator", "calc", "math"]);
    }
    if n.contains("snipping tool") || n.contains("snip & sketch") {
        push_aliases(
            &mut aliases,
            &[
                "screenshot",
                "screen capture",
                "snipping",
                "snip",
                "capture",
            ],
        );
    }
    if n.contains("photos") {
        push_aliases(
            &mut aliases,
            &["photos", "images", "pictures", "image viewer"],
        );
    }
    if n.contains("camera") {
        push_aliases(&mut aliases, &["camera", "webcam", "photo"]);
    }
    if n.contains("calendar") {
        push_aliases(&mut aliases, &["calendar", "events", "appointments"]);
    }
    if n.contains("mail") || n.contains("outlook") {
        push_aliases(&mut aliases, &["mail", "email", "e-mail"]);
    }
    if n.contains("microsoft store") || id.contains("windowsstore") {
        push_aliases(&mut aliases, &["software", "store", "apps", "app store"]);
    }
    if n.contains("task manager") {
        push_aliases(
            &mut aliases,
            &["task manager", "processes", "system monitor", "performance"],
        );
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

fn read_pipe(mut pipe: impl Read) -> Vec<u8> {
    let mut bytes = Vec::new();
    let _ = pipe.read_to_end(&mut bytes);
    bytes
}

fn get_start_apps_json(timeout: Duration) -> Result<Vec<u8>, String> {
    let script = "$ErrorActionPreference='Stop'; ConvertTo-Json -Compress -InputObject @(Get-StartApps | Select-Object Name,AppID)";
    let mut child = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start PowerShell app discovery: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture Get-StartApps stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture Get-StartApps stderr".to_string())?;
    let stdout_reader = thread::spawn(move || read_pipe(stdout));
    let stderr_reader = thread::spawn(move || read_pipe(stderr));
    let deadline = Instant::now() + timeout;

    let status = loop {
        match child
            .try_wait()
            .map_err(|e| format!("failed while waiting for Get-StartApps: {e}"))?
        {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!(
                    "Get-StartApps timed out after {} ms",
                    timeout.as_millis()
                ));
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    };

    let stdout = stdout_reader
        .join()
        .map_err(|_| "Get-StartApps stdout reader panicked".to_string())?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Get-StartApps stderr reader panicked".to_string())?;

    if !status.success() {
        return Err(format!(
            "Get-StartApps failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    Ok(stdout)
}

pub fn list() -> Result<Vec<AppEntry>, String> {
    let output = get_start_apps_json(Duration::from_secs(12))?;
    let raws: Vec<StartAppRaw> =
        serde_json::from_slice(&output).map_err(|e| format!("invalid Get-StartApps JSON: {e}"))?;

    let mut apps: Vec<AppEntry> = raws
        .into_iter()
        .filter_map(|raw| {
            let name = raw.name?.trim().to_string();
            let app_id = raw.app_id?.trim().to_string();
            if name.is_empty() || app_id.is_empty() {
                return None;
            }
            Some(AppEntry {
                aliases: aliases_for(&name, &app_id),
                name,
                app_id,
            })
        })
        .collect();

    apps.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    apps.dedup_by(|a, b| a.app_id.eq_ignore_ascii_case(&b.app_id));
    Ok(apps)
}

pub fn launch(app_id: &str) -> Result<(), String> {
    let app_id = app_id.trim();
    if app_id.is_empty() || app_id.chars().any(|c| matches!(c, '\r' | '\n' | '"')) {
        return Err("invalid application id".into());
    }

    Command::new("explorer.exe")
        .arg(format!(r"shell:AppsFolder\{app_id}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to launch application: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{aliases_for, read_pipe};

    #[test]
    fn pipe_reader_collects_all_bytes() {
        assert_eq!(read_pipe(&b"apps-json"[..]), b"apps-json");
    }

    #[test]
    fn terminal_alias_resolves_windows_terminal() {
        let aliases = aliases_for(
            "Windows Terminal",
            "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App",
        );
        assert!(aliases.iter().any(|alias| alias == "terminal"));
        assert!(aliases.iter().any(|alias| alias == "command line"));
    }

    #[test]
    fn gnome_style_names_resolve_windows_equivalents() {
        let files = aliases_for("File Explorer", "explorer.exe");
        let monitor = aliases_for("Task Manager", "TaskManager");
        let capture = aliases_for("Snipping Tool", "Microsoft.ScreenSketch_8wekyb3d8bbwe!App");

        assert!(files.iter().any(|alias| alias == "files"));
        assert!(monitor.iter().any(|alias| alias == "system monitor"));
        assert!(capture.iter().any(|alias| alias == "screenshot"));
    }
}
