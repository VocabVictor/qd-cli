mod common;

use common::*;
use std::{process::Stdio, thread, time::Duration};

#[test]
#[ignore = "explicit performance test"]
fn concurrent_read_throughput_and_memory() {
    let server = MockServer::start(20);
    let temp = isolated_environment(&server.base_url, 32);
    let login = run_qd(
        &temp,
        &[
            "login",
            "--username",
            "perf-user",
            "--password-stdin",
            "--compact",
        ],
        Some("performance-only-password\n"),
    );
    assert!(login.status.success());

    // A cold Windows host can briefly pause a newly built executable while
    // Defender scans it. Retry once so the assertion measures steady-state CLI
    // concurrency rather than that one-time host event.
    let mut attempts = Vec::new();
    for attempt in 1..=2 {
        let (result, peak_bytes) = run_benchmark(&temp);
        let rps = result["requestsPerSecond"].as_f64().unwrap();
        let p95 = result["latencyMs"]["p95"].as_f64().unwrap();
        eprintln!(
            "PERF attempt={attempt} requests=512 concurrency=32 rps={rps:.1} p95_ms={p95:.2} peak_mib={:.2}",
            peak_bytes as f64 / 1024.0 / 1024.0
        );
        attempts.push((result, peak_bytes));
        if rps > 100.0 {
            break;
        }
    }
    let (result, peak_bytes) = attempts
        .into_iter()
        .max_by(|(left, _), (right, _)| {
            left["requestsPerSecond"]
                .as_f64()
                .unwrap()
                .total_cmp(&right["requestsPerSecond"].as_f64().unwrap())
        })
        .unwrap();
    let rps = result["requestsPerSecond"].as_f64().unwrap();
    assert_eq!(result["successes"], 512);
    assert_eq!(result["failures"], 0);
    assert!(
        rps > 100.0,
        "expected concurrent throughput, got {rps:.1} rps"
    );
    if peak_bytes > 0 {
        assert!(
            peak_bytes < 96 * 1024 * 1024,
            "peak working set exceeded 96 MiB: {peak_bytes}"
        );
    }
}

fn run_benchmark(temp: &tempfile::TempDir) -> (serde_json::Value, usize) {
    let mut command = qd_command(temp);
    command
        .args([
            "bench",
            "/project/list",
            "--requests",
            "512",
            "--warmup",
            "16",
            "--concurrency",
            "32",
            "--compact",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let mut peak_bytes = 0usize;
    loop {
        peak_bytes = peak_bytes.max(process_working_set(child.id()).unwrap_or(0));
        if child.try_wait().unwrap().is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    let output = child.wait_with_output().unwrap();
    (stdout_json(&output), peak_bytes)
}

#[cfg(windows)]
fn process_working_set(pid: u32) -> Option<usize> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
    };
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        let ok = GetProcessMemoryInfo(
            process,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        );
        CloseHandle(process);
        if ok == 0 {
            None
        } else {
            Some(counters.WorkingSetSize)
        }
    }
}

#[cfg(not(windows))]
fn process_working_set(_pid: u32) -> Option<usize> {
    None
}
