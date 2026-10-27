//! 当前请求的本机用户与权限判断。session_guard 每个请求都会放一个 CurrentUser
//! 进 request extensions：公开绑定时来自 session cookie（每次重新查账号文件，
//! 所以改角色、删用户立即生效），本机回环绑定时固定为管理员。

use super::accounts::Role;
use super::ownership;
use super::state::WebError;

use axum::http::Method;

#[derive(Clone, Debug)]
pub(crate) struct CurrentUser {
    pub(crate) name: String,
    pub(crate) role: Role,
}

impl CurrentUser {
    pub(crate) fn local() -> Self {
        Self {
            name: "local".to_owned(),
            role: Role::Admin,
        }
    }

    pub(crate) fn is_admin(&self) -> bool {
        self.role == Role::Admin
    }
}

pub(crate) fn require_admin(user: &CurrentUser) -> Result<(), WebError> {
    if user.is_admin() {
        Ok(())
    } else {
        Err(WebError::forbidden("需要管理员权限"))
    }
}

/// 管理员放行；普通用户只能操作 qd web 记录在自己名下的作业。
pub(crate) fn require_owner(user: &CurrentUser, job_id: &str) -> Result<(), WebError> {
    if user.is_admin() {
        return Ok(());
    }
    match ownership::owner_of(job_id)? {
        Some(owner) if owner.eq_ignore_ascii_case(&user.name) => Ok(()),
        _ => Err(WebError::forbidden(format!(
            "作业 {job_id} 不是你通过 qd web 提交的，只能查看，不能操作"
        ))),
    }
}

/// 整条路由只给管理员的接口：平台账号登录/登出、平台配置、保活、模板写入、管理后台。
pub(crate) fn admin_only_route(method: &Method, path: &str) -> bool {
    if path.starts_with("/api/admin/") {
        return true;
    }
    if matches!(path, "/api/login" | "/api/logout" | "/api/config" | "/api/keepalive") {
        return true;
    }
    path.starts_with("/api/templates") && method != Method::GET
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_routes_covered() {
        assert!(admin_only_route(&Method::POST, "/api/logout"));
        assert!(admin_only_route(&Method::POST, "/api/config"));
        assert!(admin_only_route(&Method::GET, "/api/admin/users"));
        assert!(admin_only_route(&Method::POST, "/api/templates"));
        assert!(admin_only_route(&Method::DELETE, "/api/templates/x"));
        assert!(!admin_only_route(&Method::GET, "/api/templates"));
        assert!(!admin_only_route(&Method::GET, "/api/jobs"));
        assert!(!admin_only_route(&Method::POST, "/api/job/submit"));
    }
}
