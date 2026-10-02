use std::env;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
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
        if let Some(parent) = executable.parent() {
            command.current_dir(parent);
        }
        command
            .env("FEDORAWIN_ADWAITA_THEME", theme)
            .env("FEDORAWIN_ADWAITA_ACCENT", accent)
            .stdout(Stdio::piped());

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
