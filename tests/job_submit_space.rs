mod common;

use common::*;
use serde_json::json;
use std::{fs, sync::atomic::Ordering};

#[test]
fn job_submit_rejects_space_mismatch_before_http_and_accepts_explicit_override() {
    let server = MockServer::start(0);
    let temp = isolated_environment(&server.base_url, 1);
    let job_file = temp.path().join("job.json");
    fs::write(
        &job_file,
        serde_json::to_vec(&json!({
            "projectId": "integration-project",
            "jobName": "integration-job",
            "spaceId": "other-space"
        }))
        .unwrap(),
    )
    .unwrap();

    let rejected = qd_command(&temp)
        .env("QD_TOKEN", "integration-only-token")
        .args(["job", "submit", "--file"])
        .arg(&job_file)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let error = String::from_utf8_lossy(&rejected.stderr);
    assert!(error.contains("空间不一致"), "{error}");
    assert!(error.contains("spaceId=other-space"), "{error}");
    assert!(error.contains("实际请求空间=testspace001"), "{error}");
    assert!(error.contains("--space-id other-space"), "{error}");
    assert_eq!(server.observed.job_submit_calls.load(Ordering::Relaxed), 0);
    assert_eq!(server.observed.refreshes.load(Ordering::Relaxed), 0);

    let accepted = qd_command(&temp)
        .env("QD_TOKEN", "integration-only-token")
        .args(["job", "submit", "--file"])
        .arg(&job_file)
        .args(["--space-id", "other-space", "--compact"])
        .output()
        .unwrap();
    assert_eq!(stdout_json(&accepted)["code"], 0);
    assert_eq!(server.observed.job_submit_calls.load(Ordering::Relaxed), 1);
}
