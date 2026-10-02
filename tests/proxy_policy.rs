mod common;

use common::*;
use serde_json::Value;
use std::{fs, io::Write, process::Stdio};
use tempfile::TempDir;

fn login_with_environment_proxy(temp: &TempDir, extra: &[&str]) -> Value {
    let mut command = qd_command(temp);
    // An unreachable proxy and no exclusions make accidental inheritance fail.
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        command.env(key, "http://127.0.0.1:9");
    }
    command.env("NO_PROXY", "").env("no_proxy", "");
    let mut child = command
        .args([
            "login",
            "--username",
            "proxy-test",
            "--password-stdin",
            "--compact",
        ])
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"test-password\n")
        .unwrap();
    stdout_json(&child.wait_with_output().unwrap())
}

fn configured_proxy(temp: &TempDir) -> Value {
    let path = temp.path().join("appdata/qd/config.json");
    serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap()["proxy"].clone()
}

#[test]
fn default_ignores_environment_proxies() {
    let server = MockServer::start(0);
    let temp = isolated_environment(&server.base_url, 1);
    assert_eq!(login_with_environment_proxy(&temp, &[])["ok"], true);
}

#[test]
fn configured_proxy_routes_requests_to_explicit_proxy() {
    let proxy = MockServer::start(0);
    let temp = isolated_environment("http://qd-origin.invalid", 1);
    stdout_json(&run_qd(
        &temp,
        &["config", "set", "proxy", &proxy.base_url],
        None,
    ));
    assert_eq!(login_with_environment_proxy(&temp, &[])["ok"], true);
}

#[test]
fn cli_proxy_overrides_config_without_persisting() {
    let proxy = MockServer::start(0);
    let temp = isolated_environment("http://qd-origin.invalid", 1);
    stdout_json(&run_qd(
        &temp,
        &["config", "set", "proxy", "http://127.0.0.1:9"],
        None,
    ));
    assert_eq!(
        login_with_environment_proxy(&temp, &["--proxy", &proxy.base_url])["ok"],
        true
    );
    assert_eq!(configured_proxy(&temp), "http://127.0.0.1:9");
}

#[test]
fn cli_none_bypasses_configured_proxy_and_clear_persists() {
    let server = MockServer::start(0);
    let temp = isolated_environment(&server.base_url, 1);
    stdout_json(&run_qd(
        &temp,
        &["config", "set", "proxy", "http://127.0.0.1:9"],
        None,
    ));
    assert_eq!(
        login_with_environment_proxy(&temp, &["--proxy", "none"])["ok"],
        true
    );
    stdout_json(&run_qd(&temp, &["config", "set", "proxy", "none"], None));
    assert!(configured_proxy(&temp).is_null());
    assert_eq!(login_with_environment_proxy(&temp, &[])["ok"], true);
}

#[test]
fn config_show_hides_proxy_credentials() {
    let temp = isolated_environment("http://qd-origin.invalid", 1);
    stdout_json(&run_qd(
        &temp,
        &["config", "set", "proxy", "http://test-user:test-secret@127.0.0.1:1080"],
        None,
    ));
    let output = stdout_json(&run_qd(&temp, &["config", "show", "--compact"], None));
    assert_eq!(output["proxy"], "***REDACTED***");
}
