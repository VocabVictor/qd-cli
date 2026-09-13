use super::*;

pub(super) fn handle_request(
    mut request: Request,
    private_key: &RsaPrivateKey,
    public_pem: &str,
    observed: &Observed,
    delay_ms: u64,
) {
    let path = request
        .url()
        .split('?')
        .next()
        .unwrap_or(request.url())
        .to_owned();
    let authorization = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Authorization"))
        .map(|header| header.value.as_str().to_owned());
    let mut request_body = Vec::new();
    request.as_reader().read_to_end(&mut request_body).unwrap();

    let (status, content_type, body) = if path.ends_with("/keys/public.pem") {
        (200, "application/x-pem-file", public_pem.to_owned())
    } else if path.ends_with("/gemini_userauth/user/login") {
        let login_call = observed.login_calls.fetch_add(1, Ordering::Relaxed) + 1;
        let body: Value = serde_json::from_slice(&request_body).unwrap();
        let username = body["userName"].as_str().unwrap().to_owned();
        let cipher = STANDARD.decode(body["password"].as_str().unwrap()).unwrap();
        let password = private_key.decrypt(Pkcs1v15Encrypt, &cipher).unwrap();
        *observed.login_username.lock().unwrap() = Some(username.clone());
        *observed.login_password.lock().unwrap() = Some(String::from_utf8(password).unwrap());
        (
            200,
            "application/json",
            json!({
                "code": 0,
                "msg": "ok",
                "data": {
                    "token": if login_call == 1 { "expired-access-token" } else { "valid-access-token" },
                    "refreshToken": "test-refresh-token",
                    "userName": username
                }
            })
            .to_string(),
        )
    } else if path.ends_with("/gemini_userauth/token/refresh") {
        observed.refreshes.fetch_add(1, Ordering::Relaxed);
        if observed.fail_refresh.load(Ordering::Relaxed) {
            (
                401,
                "application/json",
                json!({"code": 401, "msg": "refresh expired"}).to_string(),
            )
        } else {
            (
                200,
                "application/json",
                json!({
                    "code": 0,
                    "msg": "ok",
                    "data": {
                        "token": "valid-access-token",
                        "refreshToken": "rotated-refresh-token"
                    }
                })
                .to_string(),
            )
        }
    } else if path.ends_with("/gemini_userauth/user/baseInfo") {
        observed.whoami_calls.fetch_add(1, Ordering::Relaxed);
        if authorization.as_deref() == Some("Bearer expired-access-token") {
            (
                401,
                "application/json",
                json!({"code": 401, "msg": "expired"}).to_string(),
            )
        } else {
            (
                200,
                "application/json",
                json!({"code": 0, "msg": "ok", "data": {"userName": "integration-user"}})
                    .to_string(),
            )
        }
    } else if path.ends_with("/user/project/list") {
        observed.project_list_calls.fetch_add(1, Ordering::Relaxed);
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        (
            200,
            "application/json",
            json!({"code": 0, "msg": "ok", "data": {"list": [], "total": 0}}).to_string(),
        )
    } else if path.ends_with("/user/job/new") {
        observed.job_submit_calls.fetch_add(1, Ordering::Relaxed);
        let body: Value = serde_json::from_slice(&request_body).unwrap();
        assert_eq!(body["projectId"], "integration-project");
        assert_eq!(body["jobName"], "integration-job");
        (
            200,
            "application/json",
            json!({"code": 0, "msg": "ok", "data": {}}).to_string(),
        )
    } else if path.ends_with("/user/project/job/list/integration-project") {
        (
            200,
            "application/json",
            json!({
                "code": 0,
                "msg": "ok",
                "data": {
                    "listCount": 1,
                    "jobList": [{
                        "jobId": "integration-job-id",
                        "jobName": "integration-job",
                        "createTime": 4102444800000_u64,
                        "status": "SUCCEEDED"
                    }]
                }
            })
            .to_string(),
        )
    } else if path.ends_with("/user/rsgroup/detail/group-1") {
        (
            200,
            "application/json",
            json!({
                "code": 0,
                "msg": "ok",
                "data": {
                    "rsgroupId": "group-1",
                    "rsgroupName": "integration-pool",
                    "nodeCount": 2,
                    "cpuTotal": 16000,
                    "memoryTotal": 65536,
                    "gpuTotal": 4,
                    "graphicsCards": "NVIDIA H800 80GB"
                }
            })
            .to_string(),
        )
    } else if path.contains("/user/jobQueue/node/job/list") {
        // Two nodes with free cards, 2 + 1, so the group total (3) deliberately
        // differs from the per-instance ceiling the stock endpoint reports (2).
        (
            200,
            "application/json",
            json!({
                "code": 0,
                "msg": "ok",
                "data": {
                    "total": 3,
                    "nodes": [
                        {"name": "node-a", "ip": "10.0.0.1", "gpuLeft": 2},
                        {"name": "node-b", "ip": "10.0.0.2", "gpuLeft": 1},
                        {"name": "node-c", "ip": "10.0.0.3", "gpuLeft": 0}
                    ]
                }
            })
            .to_string(),
        )
    } else if path.ends_with("/user/job/jobResourceStock") {
        observed
            .resource_stock_calls
            .fetch_add(1, Ordering::Relaxed);
        let body: Value = serde_json::from_slice(&request_body).unwrap();
        let resource_type = body["taskRoleList"][0]["resourceType"].as_u64().unwrap();
        (
            200,
            "application/json",
            json!({
                "code": 0,
                "message": "",
                "data": {
                    "cpu": 7,
                    "memory": 8192,
                    "gpuCount": if resource_type == 2 { 2 } else { 0 },
                    "gpus": [],
                    "leftGpuRatioCount": 0,
                    "orionScheduleRule": "local-only"
                }
            })
            .to_string(),
        )
    } else if path.contains("/user/job/cancel/") {
        observed.cancel_calls.fetch_add(1, Ordering::Relaxed);
        (
            200,
            "application/json",
            json!({"code": 0, "msg": "ok", "data": {}}).to_string(),
        )
    } else if path.contains("/user/job/detail/") {
        (
            200,
            "application/json",
            json!({"code": 0, "msg": "ok", "data": {"status": "SUCCEEDED"}}).to_string(),
        )
    } else {
        (
            404,
            "application/json",
            json!({"code": 404, "msg": path}).to_string(),
        )
    };

    let response = Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", content_type).unwrap());
    request.respond(response).unwrap();
}
