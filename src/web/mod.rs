mod accounts;
mod local_session;
mod proxy;
mod routes_auth;
mod routes_core;
mod routes_jobs;
mod routes_templates;
mod routes_workloads;
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
    let password = args
        .take_option("--password")?
        .or_else(|| std::env::var("QD_WEB_PASSWORD").ok());
    let open = args.take_flag("--open");
    args.ensure_empty()?;
    config.require_base_url()?;

    let public = host != "127.0.0.1";
    if public && password.is_none() {
        bail!(
            "绑定 {host} 会把本机登录态暴露给能连到这个端口的任何人；\
             必须用 --password 或 QD_WEB_PASSWORD 环境变量设置一个注册邀请码\
             （访问本身改用网页登录/注册，邀请码只用来防止陌生人自己注册账号）"
        );
    }
    let state = Arc::new(state::WebState::new(config, public, password));
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
    axum::serve(listener, router)
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
