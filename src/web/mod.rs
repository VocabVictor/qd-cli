mod proxy;
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
    if let Some(host) = args.take_option("--host")?
        && host != "127.0.0.1"
    {
        bail!("为了保护本机登录态，qd web 仅允许绑定 127.0.0.1");
    }
    let host = "127.0.0.1".to_owned();
    let open = args.take_flag("--open");
    args.ensure_empty()?;
    config.require_base_url()?;

    let state = Arc::new(state::WebState::new(config));
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
        .layer(middleware::from_fn(state::origin_guard))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind((host.as_str(), port))
        .await
        .with_context(|| format!("无法监听 {host}:{port}（端口可能被占用，换 --port 试试）"))?;
    let url = format!("http://{host}:{port}/");
    eprintln!("qd web 已启动: {url}  （Ctrl+C 退出）");
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
