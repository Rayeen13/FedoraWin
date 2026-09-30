use std::env;
use std::path::PathBuf;
use std::process::Command;

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

pub fn launch(theme: &str) -> Result<bool, String> {
    for executable in candidate_paths() {
        if !executable.is_file() {
            continue;
        }

        let mut command = Command::new(&executable);
        if let Some(parent) = executable.parent() {
            command.current_dir(parent);
        }
        command.env("FEDORAWIN_ADWAITA_THEME", theme);
        command
            .spawn()
            .map_err(|error| format!("failed to launch {}: {error}", executable.display()))?;
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
}
