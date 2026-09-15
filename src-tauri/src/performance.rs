use serde::Serialize;
use std::collections::HashSet;
use std::mem::{size_of, zeroed};

pub const TARGET_IDLE_MB: u64 = 100;
pub const HARD_LIMIT_MB: u64 = 300;

const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const PROCESS_VM_READ: u32 = 0x0010;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const INVALID_HANDLE_VALUE: isize = -1isize;

#[repr(C)]
struct ProcessEntry32W {
    size: u32,
    usage: u32,
    process_id: u32,
    default_heap_id: usize,
    module_id: u32,
    threads: u32,
    parent_process_id: u32,
    base_priority: i32,
    flags: u32,
    exe_file: [u16; 260],
}

#[repr(C)]
struct ProcessMemoryCounters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnapshot {
    pub working_set_mb: u64,
    pub process_count: usize,
    pub target_idle_mb: u64,
    pub hard_limit_mb: u64,
    pub within_hard_limit: bool,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> isize;
    fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
    fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> isize;
    fn CloseHandle(handle: isize) -> i32;
    fn GetCurrentProcessId() -> u32;
}

#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(process: isize, counters: *mut ProcessMemoryCounters, cb: u32) -> i32;
}

fn process_tree(root: u32) -> Result<HashSet<u32>, String> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err("CreateToolhelp32Snapshot failed".into());
    }

    let mut relationships = Vec::new();
    let mut entry: ProcessEntry32W = unsafe { zeroed() };
    entry.size = size_of::<ProcessEntry32W>() as u32;

    let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) };
    while ok != 0 {
        relationships.push((entry.process_id, entry.parent_process_id));
        entry = unsafe { zeroed() };
        entry.size = size_of::<ProcessEntry32W>() as u32;
        ok = unsafe { Process32NextW(snapshot, &mut entry) };
    }

    unsafe {
        CloseHandle(snapshot);
    }

    let mut tree = HashSet::from([root]);
    loop {
        let before = tree.len();
        for (process_id, parent_id) in &relationships {
            if tree.contains(parent_id) {
                tree.insert(*process_id);
            }
        }
        if tree.len() == before {
            break;
        }
    }
    Ok(tree)
}

fn working_set(process_id: u32) -> Option<u64> {
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, process_id) };
    if process == 0 {
        return None;
    }

    let mut counters: ProcessMemoryCounters = unsafe { zeroed() };
    counters.cb = size_of::<ProcessMemoryCounters>() as u32;
    let ok = unsafe {
        GetProcessMemoryInfo(
            process,
            &mut counters,
            size_of::<ProcessMemoryCounters>() as u32,
        )
    };
    unsafe {
        CloseHandle(process);
    }
    if ok == 0 {
        None
    } else {
        Some(counters.working_set_size as u64)
    }
}

pub fn snapshot() -> Result<MemorySnapshot, String> {
    let root = unsafe { GetCurrentProcessId() };
    let tree = process_tree(root)?;
    let bytes = tree
        .iter()
        .filter_map(|process_id| working_set(*process_id))
        .sum::<u64>();
    let working_set_mb = bytes.div_ceil(1024 * 1024);

    Ok(MemorySnapshot {
        working_set_mb,
        process_count: tree.len(),
        target_idle_mb: TARGET_IDLE_MB,
        hard_limit_mb: HARD_LIMIT_MB,
        within_hard_limit: working_set_mb <= HARD_LIMIT_MB,
    })
}

#[cfg(test)]
mod tests {
    use super::{HARD_LIMIT_MB, TARGET_IDLE_MB};

    #[test]
    fn memory_budget_keeps_hard_limit_above_target() {
        assert_eq!(TARGET_IDLE_MB, 100);
        assert_eq!(HARD_LIMIT_MB, 300);
        assert!(HARD_LIMIT_MB > TARGET_IDLE_MB);
    }
}

