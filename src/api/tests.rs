use super::*;

#[test]
fn resolves_all_service_prefixes() {
    assert_eq!(
        resolve_api_path(Service::Core, "/job/new").unwrap(),
        "/gemini/v1/gemini_api/gemini_api/user/job/new"
    );
    assert_eq!(
        resolve_api_path(Service::Core, "/user/job/event/list").unwrap(),
        "/gemini/v1/gemini_api/gemini_api/user/job/event/list"
    );
    assert_eq!(
        resolve_api_path(Service::Auth, "/user/login").unwrap(),
        "/gemini/v1/gemini_userauth/user/login"
    );
    assert_eq!(
        resolve_api_path(Service::Tool, "/user/job/event/list").unwrap(),
        "/gemini/v1/geminitool_api/geminitool_api/user/job/event/list"
    );
    assert_eq!(
        resolve_api_path(Service::Absolute, "/health").unwrap(),
        "/health"
    );
    assert!(resolve_api_path(Service::Core, "job/new").is_err());
}

#[test]
fn rejects_business_errors_without_exposing_data() {
    let error = ensure_api_success(&json!({
        "code": 40123,
        "msg": "denied",
        "data": {"token": "must-not-appear"}
    }))
    .unwrap_err()
    .to_string();
    assert!(error.contains("40123"));
    assert!(error.contains("denied"));
    assert!(!error.contains("must-not-appear"));
}

#[test]
fn sanitizes_non_json_http_errors() {
    assert_eq!(
        safe_error_message("<html>secret</html>"),
        "非 JSON 错误响应（内容已隐藏）"
    );
    assert_eq!(safe_error_message("  "), "空响应");
}
