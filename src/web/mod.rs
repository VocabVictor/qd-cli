mod accounts;
mod local_session;
mod login_override;
mod login_policy;
mod ownership;
mod policy_store;
mod proxy;
mod query;
mod quota;
mod rate_limit;
mod roles;
mod routes_access;
mod routes_admin;
mod routes_auth;
mod routes_catalog;
mod routes_core;
mod routes_jobs;
mod routes_templates;
mod routes_workloads;
mod sessions;
mod state;

use crate::*;

use axum::{
    Router, middleware,
    response::Html,
    routing::{get, post},
};
use std::sync::Arc;

const INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/index.html"));

pub(crate) async fn web_command(config: Config, args: &mut Args, _compact: bool) -> Result<()> {
    let port = option_u64(args, "--port", 7717, 1, 65535)? as u16;
    let host = args.take_option("--host")?.unwrap_or_else(|| "127.0.0.1".to_owned());
    if args.take_option("--password")?.is_some() {
        bail!("--password 已拆分为 --invite-code（普通用户）和 --admin-invite-code（管理员）");
    }
    // 推荐用环境变量传邀请码：命令行参数会出现在 ps 里。
    let mut invite = |flag: &str, env: &str| -> Result<Option<String>> {
        Ok(args
            .take_option(flag)?
            .or_else(|| std::env::var(env).ok())
            .map(|code| code.trim().to_owned())
            .filter(|code| !code.is_empty()))
    };
    let invite_user = invite("--invite-code", "QD_WEB_INVITE_CODE")?;
    let invite_admin = invite("--admin-invite-code", "QD_WEB_ADMIN_INVITE_CODE")?;
    let open = args.take_flag("--open");
    args.ensure_empty()?;
    config.require_base_url()?;

    let public = host != "127.0.0.1";
    if public {
        let (Some(user_code), Some(admin_code)) = (&invite_user, &invite_admin) else {
            bail!(
                "绑定 {host} 会把本机平台登录态暴露给能连到这个端口的人；必须同时设置 \
                 QD_WEB_INVITE_CODE（普通用户注册码）和 QD_WEB_ADMIN_INVITE_CODE（管理员注册码），\
                 也可用 --invite-code / --admin-invite-code（会出现在 ps 里，不推荐）"
            );
        };
        if user_code == admin_code {
            bail!("普通邀请码和管理员邀请码不能相同");
        }
        if user_code.chars().count() < 12 || admin_code.chars().count() < 12 {
            bail!("邀请码至少 12 位，防止被猜中");
        }
    }
    let state = Arc::new(state::WebState::new(config, public, (invite_user, invite_admin)));
    let router = Router::new()
        .route(
            "/",
            get(|| async {
                (
                    [(axum::http::header::CACHE_CONTROL, "no-cache")],
                    Html(INDEX_HTML),
                )
            }),
        )
        .route("/api/auth/state", get(routes_auth::get_state))
        .route("/api/auth/register", post(routes_auth::post_register))
        .route("/api/auth/login", post(routes_auth::post_login))
        .route("/api/auth/logout", post(routes_auth::post_logout))
        .route("/api/auth/password", post(routes_auth::post_password))
        .route("/api/admin/users", get(routes_admin::list_users))
        .route("/api/admin/users/{name}", axum::routing::delete(routes_admin::delete_user))
        .route("/api/admin/users/{name}/quota", post(routes_admin::post_quota))
        .route("/api/admin/users/{name}/role", post(routes_admin::post_role))
        .route("/api/admin/users/{name}/password", post(routes_admin::post_password))
        .route("/api/admin/users/{name}/login", post(routes_access::post_user_policy))
        .route("/api/admin/login-policy", get(routes_access::get_policy).post(routes_access::post_policy))
        .route("/api/admin/sessions", get(routes_access::list_sessions))
        .route("/api/admin/sessions/{id}", axum::routing::delete(routes_access::delete_session))
        .route("/api/state", get(routes_core::get_state))
        .route("/api/login", post(routes_core::post_login))
        .route("/api/logout", post(routes_core::post_logout))
        .route("/api/call", post(routes_core::post_call))
        .route("/api/config", post(routes_core::post_config))
        .route("/api/keepalive", post(routes_core::post_keepalive))
        .route("/api/jobs", get(routes_jobs::list_jobs))
        .route("/api/job/submit", post(routes_jobs::post_job_submit))
        .route("/api/job/{id}", get(routes_jobs::get_job))
        .route("/api/job/{id}/logs", get(routes_jobs::get_job_logs))
        .route("/api/job/{id}/metrics", get(routes_jobs::get_job_metrics))
        .route("/api/bulk", post(routes_jobs::post_bulk))
        .route("/api/devs", get(routes_workloads::list_devs))
        .route("/api/images", get(routes_catalog::list_images))
        .route("/api/nodes", get(routes_catalog::list_nodes))
        .route("/api/nodes/facets", get(routes_catalog::node_facets))
        .route("/api/idle", get(routes_workloads::get_idle))
        .route("/api/terminal-url", get(routes_workloads::get_terminal_url))
        .route("/api/exec", post(routes_workloads::post_exec))
        .route("/api/templates", get(routes_templates::list_templates).post(routes_templates::post_template))
        .route("/api/templates/{name}", axum::routing::delete(routes_templates::delete_template))
        // 终端反向代理：页面、静态资源与 WebSocket 均从本机同源提供
        .route("/terminal", get(proxy::proxy_http))
        .route("/static/{*path}", get(proxy::proxy_http))
        .route("/ws/{*path}", get(proxy::proxy_ws))
        .layer(middleware::from_fn_with_state(state.clone(), state::origin_guard))
        .layer(middleware::from_fn_with_state(state.clone(), local_session::session_guard))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port))
        .await
        .with_context(|| format!("无法监听 {host}:{port}（端口可能被占用，换 --port 试试）"))?;
    let url = format!("http://{host}:{port}/");
    eprintln!("qd web 已启动: {url}  （Ctrl+C 退出）");
    if public {
        eprintln!(
            "警告: 这是裸 HTTP，没有 TLS；登录密码和 session Cookie 在公网上能被中间人窃听。\
             只在 VPN/内网/反向代理 TLS 之后暴露这个端口，不要直接挂到公网。"
        );
    }
    if open {
        open_browser(&url);
    }
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
        .await
        .context("web 服务异常退出")
}

#[cfg(windows)]
fn open_browser(url: &str) {
    let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
}

#[cfg(not(windows))]
fn open_browser(url: &str) {
    let _ = Command::new("xdg-open").arg(url).spawn();
}
