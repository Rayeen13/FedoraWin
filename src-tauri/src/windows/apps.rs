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
            &[
                "task manager",
                "processes",
                "system monitor",
                "performance",
            ],
        );
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

pub fn list() -> Result<Vec<AppEntry>, String> {
    let script = "$ErrorActionPreference='Stop'; ConvertTo-Json -Compress -InputObject @(Get-StartApps | Select-Object Name,AppID)";
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
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
    use super::aliases_for;

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
        let capture = aliases_for(
            "Snipping Tool",
            "Microsoft.ScreenSketch_8wekyb3d8bbwe!App",
        );

        assert!(files.iter().any(|alias| alias == "files"));
        assert!(monitor.iter().any(|alias| alias == "system monitor"));
        assert!(capture.iter().any(|alias| alias == "screenshot"));
    }
}
