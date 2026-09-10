use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

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

fn aliases_for(name: &str, app_id: &str) -> Vec<String> {
    let n = name.to_ascii_lowercase();
    let id = app_id.to_ascii_lowercase();
    let mut aliases = Vec::new();

    if n.contains("terminal") || id.contains("windowsterminal") {
        aliases.extend(["terminal", "term", "console", "shell"].map(str::to_string));
    }
    if n.contains("powershell") {
        aliases.extend(["terminal", "shell", "pwsh", "powershell"].map(str::to_string));
    }
    if n.contains("command prompt") || n == "cmd" || id.ends_with("cmd.exe") {
        aliases.extend(["terminal", "console", "cmd", "command prompt"].map(str::to_string));
    }
    if n.contains("file explorer") || n == "explorer" {
        aliases.extend(["files", "file manager", "explorer", "folders"].map(str::to_string));
    }
    if n.contains("settings") {
        aliases.extend(["settings", "preferences", "control panel"].map(str::to_string));
    }
    if ["edge", "chrome", "firefox", "opera", "brave"].iter().any(|b| n.contains(b)) {
        aliases.extend(["browser", "web", "internet"].map(str::to_string));
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

pub fn list() -> Result<Vec<AppEntry>, String> {
    let script = "$ErrorActionPreference='Stop'; ConvertTo-Json -Compress -InputObject @(Get-StartApps | Select-Object Name,AppID)";
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("failed to start PowerShell app discovery: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Get-StartApps failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let raws: Vec<StartAppRaw> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("invalid Get-StartApps JSON: {e}"))?;

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

    apps.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
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
