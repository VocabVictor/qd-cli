use super::*;

#[test]
fn argument_parser_supports_equals_and_separate_values() {
    let mut args = Args::new(
        ["job", "--page=2", "--size", "50", "--compact"]
            .into_iter()
            .map(str::to_owned),
    );
    assert_eq!(args.pop().as_deref(), Some("job"));
    assert_eq!(args.take_option("--page").unwrap().as_deref(), Some("2"));
    assert_eq!(args.take_option("--size").unwrap().as_deref(), Some("50"));
    assert!(args.take_flag("--compact"));
    args.ensure_empty().unwrap();
}

#[test]
fn terminal_status_detection_is_strict() {
    assert!(result_is_terminal(&json!({
        "ok": true,
        "response": {"data": {"status": "SUCCEEDED"}}
    })));
    assert!(!result_is_terminal(&json!({
        "ok": true,
        "response": {"data": {"status": "RUNNING"}}
    })));
    assert!(result_is_terminal(&json!({"ok": false})));
}

#[test]
fn percentile_handles_edges() {
    assert_eq!(percentile_us(&[], 99), 0);
    assert_eq!(percentile_us(&[10], 99), 10);
    assert_eq!(percentile_us(&[1, 2, 3, 4, 5], 50), 3);
    assert_eq!(percentile_us(&[1, 2, 3, 4, 5], 99), 5);
}

#[test]
fn ids_reject_path_injection() {
    assert!(validate_id("1234567890", "JOB_ID").is_ok());
    assert!(validate_id("../admin", "JOB_ID").is_err());
    assert!(validate_id("1?x=2", "JOB_ID").is_err());
}

#[test]
fn sensitive_fields_are_redacted_recursively() {
    let mut value = json!({
        "data": {
            "token": "access-token",
            "codeRepositoryToken": "repository-token",
            "instanceList": [{
                "terminalUrl": "https://example.test/terminal?token=terminal-token",
                "status": "RUNNING"
            }]
        }
    });
    redact_sensitive_fields(&mut value);
    assert_eq!(value["data"]["token"], "***REDACTED***");
    assert_eq!(value["data"]["codeRepositoryToken"], "***REDACTED***");
    assert_eq!(
        value["data"]["instanceList"][0]["terminalUrl"],
        "***REDACTED***"
    );
    assert_eq!(value["data"]["instanceList"][0]["status"], "RUNNING");
}

#[test]
fn web_terminal_protocol_extracts_stdout_only() {
    let stdout = terminal_output(Message::Text(
        json!({"operation": "stdout", "data": "hello\r\n"})
            .to_string()
            .into(),
    ));
    assert_eq!(stdout.as_deref(), Some("hello\r\n"));
    let ignored = terminal_output(Message::Text(
        json!({"operation": "resize", "cols": 80})
            .to_string()
            .into(),
    ));
    assert!(ignored.is_none());
}

#[test]
fn web_terminal_quotes_commands_and_strips_control_sequences() {
    assert_eq!(shell_quote("echo"), "echo");
    assert_eq!(shell_quote("hello world"), "'hello world'");
    assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
    assert_eq!(
        strip_terminal_control("\u{1b}[31mred\u{1b}[0m\r\nok"),
        "red\nok"
    );
}

#[test]
fn graphics_card_list_supports_platform_separators() {
    assert_eq!(
        parse_graphics_cards("NVIDIA H800 80GB, NVIDIA A100|NVIDIA L40、NVIDIA T4"),
        vec!["NVIDIA H800 80GB", "NVIDIA A100", "NVIDIA L40", "NVIDIA T4"]
    );
    assert!(parse_graphics_cards("").is_empty());
}

#[test]
fn table_width_handles_chinese_and_emoji() {
    assert_eq!(display_width("资源组"), 6);
    assert_eq!(display_width("GPU 可用"), 8);
    assert_eq!(display_width("✅"), 2);
}

#[test]
fn table_cells_are_truncated_by_display_width() {
    let value = truncate_display("NVIDIA H800 80GB（空闲）", 14);
    assert!(display_width(&value) <= 14);
    assert!(value.ends_with('…'));
}

#[test]
fn storage_units_are_human_readable() {
    assert_eq!(format_memory_mib(8192.0), "8.00 GiB");
    assert_eq!(format_memory_mib(1_718_763.0), "1.64 TiB");
    assert_eq!(format_bytes(1024.0 * 1024.0 * 3.0), "3.00 MiB");
}

#[test]
fn dev_summary_keeps_status_and_drops_large_nested_payloads() {
    let response = json!({
        "code": 0,
        "data": {
            "jobenvId": 42,
            "jobenvName": "rollout",
            "spaceName": "team-b",
            "status": "RUNNING",
            "subStatus": "INIT",
            "imageDesc": "vllm:29b",
            "tools": ["jupyterlab", "ssh"],
            "services": [{"serviceId": 1}],
            "devJobDetail": {
                "jobId": "9876543210",
                "taskroleList": [{
                    "instanceList": [{
                        "terminalUrl": "https://example.test/very-large-secret-url"
                    }]
                }]
            }
        }
    });
    let summary = dev_summary(&response);
    assert_eq!(summary["id"], "42");
    assert_eq!(summary["status"], "RUNNING / INIT");
    assert_eq!(summary["tools"], "jupyterlab, ssh");
    assert_eq!(summary["services"], 1);
    assert!(summary.pointer("/devJobDetail").is_none());
    assert!(serde_json::to_string(&summary).unwrap().len() < 350);
}

#[test]
fn owned_scope_is_the_default() {
    let mut args = Args::new(std::iter::empty());
    assert_eq!(project_scope(&mut args).unwrap(), "mine");

    let mut query = json!({});
    apply_project_scope(&mut query, "mine").unwrap();
    assert_eq!(query["accessType"], 1);

    let mut all = json!({});
    apply_project_scope(&mut all, "all").unwrap();
    assert!(all.get("accessType").is_none());
}

#[test]
fn node_resource_filters_match_the_web_queue_api() {
    let mut args = Args::new(
        [
            "--group",
            "group-1",
            "--gpu-type",
            "physical",
            "--gpu-model",
            "NVIDIA H800 80GB",
            "--min-gpu",
            "2",
            "--min-cpu",
            "8",
            "--min-memory-gb",
            "64",
            "--sort",
            "memory",
            "--asc",
            "--page",
            "3",
            "--size",
            "25",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    let query = node_resource_query(&mut args).unwrap();
    args.ensure_empty().unwrap();
    assert_eq!(query["rsgroup"], "group-1");
    assert_eq!(query["gpuVStat"], 1);
    assert_eq!(query["gpuModel"], "NVIDIA H800 80GB");
    assert_eq!(query["leftGpuCount"], 2);
    assert_eq!(query["leftCpu"], 8000);
    assert_eq!(query["leftMemory"], 65536);
    assert_eq!(query["sortField"], "left_memory");
    assert_eq!(query["sortOrder"], "asc");
    assert_eq!(query["pageNum"], 3);
    assert_eq!(query["pageSize"], 25);
}

#[test]
fn node_resource_filters_reject_conflicting_sort_direction() {
    let mut args = Args::new(["--asc", "--desc"].into_iter().map(str::to_owned));
    assert!(node_resource_query(&mut args).is_err());
}
