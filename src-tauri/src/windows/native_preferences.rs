use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

const APPEARANCE_PREFIX: &str = "FEDORAWIN_APPEARANCE";

fn push_unique(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !paths.iter().any(|existing| existing == &candidate) {
        paths.push(candidate);
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(configured) = env::var_os("FEDORAWIN_NATIVE_PREFERENCES_EXE") {
        if !configured.is_empty() {
            push_unique(&mut paths, PathBuf::from(configured));
        }
    }

    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            // Shipping layout keeps GTK/libadwaita isolated from the lightweight
            // shell so those DLLs are loaded only when Preferences is opened.
            push_unique(
                &mut paths,
                parent
                    .join("preferences-runtime")
                    .join("fedorawin-preferences.exe"),
            );
            // Keep the adjacent executable path as a compatibility fallback for
            // development builds and older staging layouts.
            push_unique(&mut paths, parent.join("fedorawin-preferences.exe"));
        }
    }

    // Development fallback: native/adwaita is built independently so GTK never
    // becomes an idle dependency of the Rust/Win32 shell.
    push_unique(
        &mut paths,
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("native")
            .join("adwaita")
            .join("out")
            .join("fedorawin-preferences.exe"),
    );

    paths
}

fn is_portable_runtime(executable: &Path) -> bool {
    executable
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("preferences-runtime"))
}

fn configure_command(command: &mut Command, executable: &Path, theme: &str, accent: &str) {
    if let Some(parent) = executable.parent() {
        command.current_dir(parent);
        if is_portable_runtime(executable) {
            let share = parent.join("share");
            command
                .env("GDK_BACKEND", "win32")
                .env("GSETTINGS_BACKEND", "memory")
                .env(
                    "GSETTINGS_SCHEMA_DIR",
                    share.join("glib-2.0").join("schemas"),
                )
                .env("XDG_DATA_DIRS", &share);
        }
    }

    command
        .env("FEDORAWIN_ADWAITA_THEME", theme)
        .env("FEDORAWIN_ADWAITA_ACCENT", accent)
        .stdout(Stdio::piped());
}

fn parse_appearance_line(line: &str) -> Option<(String, String)> {
    let mut fields = line.trim_end().split('\t');
    if fields.next()? != APPEARANCE_PREFIX {
        return None;
    }
    let theme = fields.next()?.to_owned();
    let accent = fields.next()?.to_owned();
    if fields.next().is_some() {
        return None;
    }
    Some((theme, accent))
}

pub fn launch<F>(theme: &str, accent: &str, on_appearance: F) -> Result<bool, String>
where
    F: Fn(String, String) + Send + 'static,
{
    for executable in candidate_paths() {
        if !executable.is_file() {
            continue;
        }

        let mut command = Command::new(&executable);
        configure_command(&mut command, &executable, theme, accent);

        let mut child = command
            .spawn()
            .map_err(|error| format!("failed to launch {}: {error}", executable.display()))?;
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.kill();
                return Err("native Preferences stdout pipe was unavailable".into());
            }
        };

        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some((theme, accent)) = parse_appearance_line(&line) {
                    on_appearance(theme, accent);
                }
            }
        });
        return Ok(true);
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_candidate_uses_expected_binary_name() {
        let candidates = candidate_paths();
        assert!(candidates.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("fedorawin-preferences.exe"))
        }));
        assert!(candidates.iter().any(|path| {
            path.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("preferences-runtime"))
        }));
    }

    fn command_env(command: &Command, key: &str) -> Option<String> {
        command
            .get_envs()
            .find(|(name, _)| name.to_string_lossy().eq_ignore_ascii_case(key))
            .and_then(|(_, value)| value)
            .map(|value| value.to_string_lossy().into_owned())
    }

    #[test]
    fn portable_runtime_configures_local_gtk_data_without_global_path_dependency() {
        let executable =
            PathBuf::from(r"C:\FedoraWin\preferences-runtime\fedorawin-preferences.exe");
        let mut command = Command::new(&executable);
        configure_command(&mut command, &executable, "dark", "purple");

        assert_eq!(
            command.get_current_dir(),
            executable.parent(),
            "portable Preferences must run from its isolated runtime directory"
        );
        assert_eq!(
            command_env(&command, "GDK_BACKEND").as_deref(),
            Some("win32")
        );
        assert_eq!(
            command_env(&command, "GSETTINGS_BACKEND").as_deref(),
            Some("memory")
        );
        assert_eq!(
            command_env(&command, "GSETTINGS_SCHEMA_DIR").as_deref(),
            Some(r"C:\FedoraWin\preferences-runtime\share\glib-2.0\schemas")
        );
        assert_eq!(
            command_env(&command, "XDG_DATA_DIRS").as_deref(),
            Some(r"C:\FedoraWin\preferences-runtime\share")
        );
        assert_eq!(
            command_env(&command, "FEDORAWIN_ADWAITA_THEME").as_deref(),
            Some("dark")
        );
        assert_eq!(
            command_env(&command, "FEDORAWIN_ADWAITA_ACCENT").as_deref(),
            Some("purple")
        );
        assert_eq!(command_env(&command, "PATH"), None);
    }

    #[test]
    fn adjacent_development_preferences_does_not_override_gtk_data_roots() {
        let executable = PathBuf::from(r"C:\FedoraWin\fedorawin-preferences.exe");
        let mut command = Command::new(&executable);
        configure_command(&mut command, &executable, "light", "blue");

        assert_eq!(command_env(&command, "GSETTINGS_SCHEMA_DIR"), None);
        assert_eq!(command_env(&command, "XDG_DATA_DIRS"), None);
        assert_eq!(command_env(&command, "GDK_BACKEND"), None);
    }

    #[test]
    fn appearance_protocol_accepts_exact_three_field_messages() {
        assert_eq!(
            parse_appearance_line("FEDORAWIN_APPEARANCE\tdark\tpurple"),
            Some(("dark".into(), "purple".into()))
        );
        assert_eq!(
            parse_appearance_line("FEDORAWIN_APPEARANCE\tlight\tblue\n"),
            Some(("light".into(), "blue".into()))
        );
    }

    #[test]
    fn appearance_protocol_ignores_unrelated_or_malformed_output() {
        assert_eq!(parse_appearance_line("GTK warning"), None);
        assert_eq!(parse_appearance_line("FEDORAWIN_APPEARANCE\tdark"), None);
        assert_eq!(
            parse_appearance_line("FEDORAWIN_APPEARANCE\tdark\tblue\textra"),
            None
        );
    }
}
