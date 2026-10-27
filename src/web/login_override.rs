//! 单个账号对全局登录策略的覆盖：每一项都可以单独选择跟随全局或自己设置，
//! 白名单还能在全局基础上追加。最终生效的策略由 `resolve` 合并得出。

use super::login_policy::{self, BindKind, LoginPolicy, SessionMode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ListMode {
    /// 直接用全局白名单
    #[default]
    Inherit,
    /// 全局白名单 + 这里的条目
    Extend,
    /// 只用这里的条目（留空表示这一项对该账号不限制）
    Replace,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ListRule {
    pub(crate) mode: ListMode,
    pub(crate) items: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct LoginOverride {
    /// 完全不受登录限制（和管理员一样）
    pub(crate) exempt: bool,
    /// None 跟随全局
    pub(crate) session_mode: Option<SessionMode>,
    /// None 跟随全局；Some(空) 表示该账号不做环境绑定
    pub(crate) bind: Option<Vec<BindKind>>,
    pub(crate) ip_allow: ListRule,
    pub(crate) device_allow: ListRule,
}

impl LoginOverride {
    /// 全部跟随全局时没必要存。
    pub(crate) fn is_noop(&self) -> bool {
        *self == Self::default()
    }
}

fn merge(global: &[String], rule: &ListRule) -> Vec<String> {
    match rule.mode {
        ListMode::Inherit => global.to_vec(),
        ListMode::Replace => rule.items.clone(),
        ListMode::Extend if global.is_empty() => Vec::new(),
        ListMode::Extend => {
            let mut all = global.to_vec();
            all.extend(rule.items.iter().filter(|item| !global.contains(item)).cloned());
            all
        }
    }
}

/// 合并出这个账号最终生效的策略；exempt 时返回 None（不受限）。
pub(crate) fn resolve(global: &LoginPolicy, own: Option<&LoginOverride>) -> Option<LoginPolicy> {
    let Some(own) = own else { return Some(global.clone()) };
    if own.exempt {
        return None;
    }
    Some(LoginPolicy {
        session_mode: own.session_mode.unwrap_or(global.session_mode),
        bind: own.bind.clone().unwrap_or_else(|| global.bind.clone()),
        ip_allow: merge(&global.ip_allow, &own.ip_allow),
        device_allow: merge(&global.device_allow, &own.device_allow),
    })
}

/// 校验管理员提交的覆盖：条目格式复用全局策略的校验。
pub(crate) fn normalize(mut own: LoginOverride) -> Result<LoginOverride, String> {
    let probe = login_policy::normalize(LoginPolicy {
        session_mode: SessionMode::Off,
        bind: own.bind.clone().unwrap_or_default(),
        ip_allow: own.ip_allow.items.clone(),
        device_allow: own.device_allow.items.clone(),
    })?;
    if own.bind.is_some() {
        own.bind = Some(probe.bind);
    }
    own.ip_allow.items = probe.ip_allow;
    own.device_allow.items = probe.device_allow;
    if own.ip_allow.mode == ListMode::Inherit {
        own.ip_allow.items.clear();
    }
    if own.device_allow.mode == ListMode::Inherit {
        own.device_allow.items.clear();
    }
    Ok(own)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn global() -> LoginPolicy {
        LoginPolicy {
            session_mode: SessionMode::Kick,
            bind: vec![BindKind::Agent],
            ip_allow: vec!["10.0.0.0/8".into()],
            device_allow: vec![],
        }
    }

    #[test]
    fn follows_global_by_default() {
        assert_eq!(resolve(&global(), None), Some(global()));
        assert_eq!(resolve(&global(), Some(&LoginOverride::default())), Some(global()));
        assert!(resolve(&global(), Some(&LoginOverride { exempt: true, ..Default::default() })).is_none());
    }

    #[test]
    fn per_item_override() {
        let own = LoginOverride {
            session_mode: Some(SessionMode::Deny),
            ip_allow: ListRule { mode: ListMode::Extend, items: vec!["192.168.1.7".into()] },
            device_allow: ListRule { mode: ListMode::Replace, items: vec!["abcdef12".into()] },
            ..Default::default()
        };
        let got = resolve(&global(), Some(&own)).unwrap();
        assert_eq!(got.session_mode, SessionMode::Deny);
        assert_eq!(got.bind, vec![BindKind::Agent]);
        assert_eq!(got.ip_allow, vec!["10.0.0.0/8".to_owned(), "192.168.1.7".to_owned()]);
        assert_eq!(got.device_allow, vec!["abcdef12".to_owned()]);
    }

    #[test]
    fn replace_empty_lifts_limit() {
        let own = LoginOverride {
            bind: Some(vec![]),
            ip_allow: ListRule { mode: ListMode::Replace, items: vec![] },
            ..Default::default()
        };
        let got = resolve(&global(), Some(&own)).unwrap();
        assert!(got.bind.is_empty());
        assert!(got.ip_allow.is_empty());
        // 全局不限 IP 时，追加没有意义：仍然不限
        let open = LoginPolicy { ip_allow: vec![], ..global() };
        let extend = LoginOverride { ip_allow: ListRule { mode: ListMode::Extend, items: vec!["1.2.3.4".into()] }, ..Default::default() };
        assert!(resolve(&open, Some(&extend)).unwrap().ip_allow.is_empty());
    }

    #[test]
    fn normalize_checks_items() {
        let bad = LoginOverride { ip_allow: ListRule { mode: ListMode::Extend, items: vec!["nope".into()] }, ..Default::default() };
        assert!(normalize(bad).is_err());
        let inherit = LoginOverride { device_allow: ListRule { mode: ListMode::Inherit, items: vec!["abcdef12".into()] }, ..Default::default() };
        assert!(normalize(inherit).unwrap().is_noop());
    }
}
