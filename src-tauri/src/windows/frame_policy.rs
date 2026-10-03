//! Persistent, user-controlled exclusions for foreign-window DWM styling.
//! Stores only local executable paths under the current user's LocalAppData.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const MOVEFILE_REPLACE_EXISTING: u32 = 1;
const MOVEFILE_WRITE_THROUGH: u32 = 8;
const POLICY_VERSION: u32 = 1;
static POLICY: OnceLock<Result<Mutex<HashSet<String>>, String>> = OnceLock::new();

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn QueryFullProcessImageNameW(
        process: isize,
        flags: u32,
        image_name: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPolicy {
    version: u32,
    executables: Vec<String>,
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim()
        .trim_end_matches('\\')
        .to_lowercase()
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn policy_path() -> Result<PathBuf, String> {
    let root = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        "LOCALAPPDATA is unavailable; persistent frame policy is disabled".to_string()
    })?;
    Ok(PathBuf::from(root)
        .join("FedoraWin")
        .join("frame-exclusions.json"))
}

fn load_policy() -> Result<Mutex<HashSet<String>>, String> {
    let path = policy_path()?;
    if !path.exists() {
        return Ok(Mutex::new(HashSet::new()));
    }
    let bytes =
        fs::read(&path).map_err(|e| format!("could not read persistent frame exclusions: {e}"))?;
    let stored: StoredPolicy = serde_json::from_slice(&bytes)
        .map_err(|e| format!("persistent frame exclusions are invalid: {e}"))?;
    if stored.version != POLICY_VERSION {
        return Err(format!(
            "unsupported persistent frame exclusion policy version {}",
            stored.version
        ));
    }
    Ok(Mutex::new(
        stored
            .executables
            .into_iter()
            .map(|v| normalize_path(Path::new(&v)))
            .filter(|v| !v.is_empty())
            .collect(),
    ))
}

fn policy() -> Result<&'static Mutex<HashSet<String>>, String> {
    match POLICY.get_or_init(load_policy) {
        Ok(policy) => Ok(policy),
        Err(error) => Err(error.clone()),
    }
}

fn persist_policy(executables: &HashSet<String>) -> Result<(), String> {
    let path = policy_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| "persistent frame policy has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("could not create FedoraWin policy directory: {e}"))?;
    let mut values: Vec<String> = executables.iter().cloned().collect();
    values.sort_unstable();
    let data = serde_json::to_vec_pretty(&StoredPolicy {
        version: POLICY_VERSION,
        executables: values,
    })
    .map_err(|e| e.to_string())?;
    let pending = path.with_extension("pending");
    let mut file = File::create(&pending)
        .map_err(|e| format!("could not create pending frame policy: {e}"))?;
    file.write_all(&data)
        .map_err(|e| format!("could not write pending frame policy: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("could not flush pending frame policy: {e}"))?;
    drop(file);
    let moved = unsafe {
        MoveFileExW(
            wide(&pending).as_ptr(),
            wide(&path).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(format!(
            "atomic frame exclusion policy replace failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

pub fn process_key(pid: u32) -> Result<String, String> {
    if pid == 0 {
        return Err("invalid process id".into());
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process == 0 {
        return Err(format!(
            "could not inspect target process {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
    unsafe { CloseHandle(process) };
    if ok == 0 || length == 0 {
        return Err(format!(
            "could not resolve target executable for process {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let key = normalize_path(&PathBuf::from(String::from_utf16_lossy(
        &buffer[..length as usize],
    )));
    if key.is_empty() {
        return Err("target executable path is empty".into());
    }
    Ok(key)
}

pub fn is_pid_excluded(pid: u32) -> Result<bool, String> {
    let key = process_key(pid)?;
    Ok(policy()?
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(&key))
}

pub fn set_pid_excluded(pid: u32, excluded: bool) -> Result<bool, String> {
    let key = process_key(pid)?;
    let policy = policy()?;
    let mut values = policy.lock().unwrap_or_else(|e| e.into_inner());
    let changed = if excluded {
        values.insert(key.clone())
    } else {
        values.remove(&key)
    };
    if !changed {
        return Ok(false);
    }
    if let Err(error) = persist_policy(&values) {
        if excluded {
            values.remove(&key);
        } else {
            values.insert(key);
        }
        return Err(error);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{normalize_path, StoredPolicy, POLICY_VERSION};
    use std::path::Path;

    #[test]
    fn executable_keys_are_case_and_separator_insensitive() {
        assert_eq!(
            normalize_path(Path::new(r"C:/Apps/Foo/Foo.EXE")),
            normalize_path(Path::new(r"c:\apps\foo\foo.exe"))
        );
    }

    #[test]
    fn stored_policy_is_versioned() {
        let value = serde_json::to_value(StoredPolicy {
            version: POLICY_VERSION,
            executables: vec![r"c:\a.exe".to_string()],
        })
        .expect("serialize frame policy");
        assert_eq!(value["version"], 1);
    }
}
