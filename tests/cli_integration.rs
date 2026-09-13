mod common;

use common::*;
use serde_json::Value;
use std::fs;
use std::sync::atomic::Ordering;

#[test]
fn login_refresh_read_and_bounded_bulk_flow() {
    let server = MockServer::start(0);
    let temp = isolated_environment(&server.base_url, 16);
    let test_password = "integration-only-password";

    let login = run_qd(
        &temp,
        &[
            "login",
            "--username",
            "integration-user",
            "--password-stdin",
            "--compact",
        ],
        Some(&format!("{test_password}\n")),
    );
    let login_json = stdout_json(&login);
    assert_eq!(login_json["ok"], true);
    let combined_output = format!(
        "{}{}",
        String::from_utf8_lossy(&login.stdout),
        String::from_utf8_lossy(&login.stderr)
    );
    assert!(!combined_output.contains(test_password));
    assert!(!combined_output.contains("expired-access-token"));
    assert_file_exists(&session_file(&temp));
    assert_file_exists(&credentials_file(&temp));
    assert_eq!(
        server.observed.login_username.lock().unwrap().as_deref(),
        Some("integration-user")
    );
    assert_eq!(
        server.observed.login_password.lock().unwrap().as_deref(),
        Some(test_password)
    );

    let whoami = stdout_json(&run_qd(&temp, &["whoami", "--compact"], None));
    assert_eq!(whoami["data"]["userName"], "integration-user");
    assert_eq!(server.observed.refreshes.load(Ordering::Relaxed), 1);
    assert_eq!(server.observed.whoami_calls.load(Ordering::Relaxed), 2);

    let projects = stdout_json(&run_qd(&temp, &["project", "list", "--compact"], None));
    assert_eq!(projects["data"]["total"], 0);

    let my_jobs = stdout_json(&run_qd(&temp, &["job", "list", "--compact"], None));
    assert_eq!(my_jobs["data"]["listCount"], 0);
    assert_eq!(my_jobs["data"]["scope"], "mine");

    let my_dev = stdout_json(&run_qd(&temp, &["dev", "list", "--compact"], None));
    assert_eq!(my_dev["data"]["listCount"], 0);
    assert_eq!(my_dev["data"]["scope"], "mine");

    let ids: Vec<String> = (1..=16).map(|id| id.to_string()).collect();
    let mut owned_args = vec!["job".to_owned(), "cancel".to_owned()];
    owned_args.extend(ids);
    owned_args.extend(["--concurrency".into(), "4".into(), "--compact".into()]);
    let refs: Vec<&str> = owned_args.iter().map(String::as_str).collect();
    let cancelled: Value = stdout_json(&run_qd(&temp, &refs, None));
    assert_eq!(cancelled.as_array().unwrap().len(), 16);
    let failures: Vec<&Value> = cancelled
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["ok"] != true)
        .collect();
    assert!(failures.is_empty(), "bulk failures: {failures:#?}");
    assert_eq!(server.observed.cancel_calls.load(Ordering::Relaxed), 16);

    let job_file = temp.path().join("job.json");
    fs::write(
        &job_file,
        serde_json::to_vec(&serde_json::json!({
            "projectId": "integration-project",
            "jobName": "integration-job"
        }))
        .unwrap(),
    )
    .unwrap();
    let job_file = job_file.to_string_lossy().into_owned();
    let submitted = run_qd(
        &temp,
        &[
            "job",
            "submit",
            "--file",
            &job_file,
            "--wait",
            "--interval",
            "1",
            "--timeout",
            "5",
            "--compact",
        ],
        None,
    );
    assert!(
        submitted.status.success(),
        "qd failed: {}",
        String::from_utf8_lossy(&submitted.stderr)
    );
    let lines: Vec<Value> = String::from_utf8(submitted.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["data"]["jobId"], "integration-job-id");
    assert_eq!(lines[1][0]["response"]["data"]["status"], "SUCCEEDED");
    assert_eq!(server.observed.job_submit_calls.load(Ordering::Relaxed), 1);

    let idle = stdout_json(&run_qd(&temp, &["idle", "group-1", "--compact"], None));
    assert_eq!(idle["data"]["listCount"], 1);
    assert_eq!(idle["data"]["groups"][0]["available"], true);
    assert_eq!(idle["data"]["groups"][0]["gpuAvailable"], true);
    assert_eq!(idle["data"]["groups"][0]["idle"]["cpuCores"], 7);
    assert_eq!(idle["data"]["groups"][0]["idle"]["gpus"][0]["count"], 2);
    // Group total comes from the node list and must not be confused with the
    // per-instance ceiling above.
    assert_eq!(idle["data"]["groups"][0]["idle"]["gpuWholeCardsFree"], 3);
    assert_eq!(idle["data"]["groups"][0]["idle"]["gpuMaxOnOneNode"], 2);
    assert_eq!(idle["data"]["groups"][0]["idle"]["nodesWithFreeGpu"], 2);

    let idle_table = run_qd(&temp, &["idle", "group-1"], None);
    assert!(idle_table.status.success());
    let idle_table = String::from_utf8(idle_table.stdout).unwrap();
    assert!(idle_table.contains('┌'));
    assert!(idle_table.contains("资源组"));
    assert!(idle_table.contains("integration-pool"));
    assert!(idle_table.contains("整组 3 / 单实例 2"));
    assert!(idle_table.contains("分布在 2 个节点"));
    assert_eq!(
        server.observed.resource_stock_calls.load(Ordering::Relaxed),
        4
    );
}

#[test]
fn dpapi_credentials_relogin_after_refresh_expiry() {
    let server = MockServer::start(0);
    server.observed.fail_refresh.store(true, Ordering::Relaxed);
    let temp = isolated_environment(&server.base_url, 4);
    let test_password = "auto-login-only-password";

    let login = run_qd(
        &temp,
        &[
            "login",
            "--username",
            "integration-user",
            "--password-stdin",
            "--compact",
        ],
        Some(&format!("{test_password}\n")),
    );
    assert_eq!(stdout_json(&login)["autoLogin"], true);
    assert_file_exists(&credentials_file(&temp));

    let whoami = run_qd(&temp, &["whoami", "--compact"], None);
    let whoami_json = stdout_json(&whoami);
    assert_eq!(whoami_json["data"]["userName"], "integration-user");
    assert_eq!(server.observed.refreshes.load(Ordering::Relaxed), 1);
    assert_eq!(server.observed.login_calls.load(Ordering::Relaxed), 2);
    let combined_output = format!(
        "{}{}",
        String::from_utf8_lossy(&whoami.stdout),
        String::from_utf8_lossy(&whoami.stderr)
    );
    assert!(!combined_output.contains(test_password));

    let logout = stdout_json(&run_qd(&temp, &["logout", "--compact"], None));
    assert_eq!(logout["sessionRemoved"], true);
    assert_eq!(logout["credentialRemoved"], true);
    assert!(!session_file(&temp).exists());
    assert!(!credentials_file(&temp).exists());
}
