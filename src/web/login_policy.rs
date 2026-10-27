//! 普通用户的登录限制（管理员不受限）：单点登录、会话绑定（IP / 网段 / 浏览器 / 设备指纹）、
//! IP 白名单与设备白名单。白名单在登录时和之后每个请求都检查，管理员改策略立即生效。
//!
//! 设备指纹由前端算出放在 X-QD-Device 头里，客户端可以伪造：它能挡住 cookie 被偷后换台机器用，
//! 挡不住专门研究过指纹算法的人。真正的硬边界是 IP 白名单。

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub(crate) const DEVICE_HEADER: &str = "x-qd-device";
const MAX_ENTRIES: usize = 64;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SessionMode {
    /// 不限制同时在线
    Off,
    /// 新登录把旧会话踢下线
    #[default]
    Kick,
    /// 已有活跃会话时拒绝新登录
    Deny,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum BindKind {
    Ip,
    Subnet,
    Agent,
    Device,
}

impl BindKind {
    fn label(self) -> &'static str {
        match self {
            Self::Ip => "IP 地址",
            Self::Subnet => "网段",
            Self::Agent => "浏览器",
            Self::Device => "设备",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct LoginPolicy {
    pub(crate) session_mode: SessionMode,
    pub(crate) bind: Vec<BindKind>,
    /// 单个 IP 或 CIDR（10.0.0.0/8、2001:db8::/32）；空表示不限制
    pub(crate) ip_allow: Vec<String>,
    /// 设备指纹；空表示不限制
    pub(crate) device_allow: Vec<String>,
}

impl Default for LoginPolicy {
    fn default() -> Self {
        Self {
            session_mode: SessionMode::Kick,
            bind: vec![BindKind::Agent, BindKind::Device],
            ip_allow: Vec::new(),
            device_allow: Vec::new(),
        }
    }
}

/// 一次请求的来源环境。
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClientInfo {
    pub(crate) ip: IpAddr,
    pub(crate) agent: String,
    pub(crate) device: String,
}

fn valid_device(text: &str) -> bool {
    (8..=64).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub(crate) fn client_from(peer: Option<IpAddr>, headers: &HeaderMap) -> ClientInfo {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let device = header(DEVICE_HEADER).trim();
    ClientInfo {
        ip: peer.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)).to_canonical(),
        agent: header("user-agent").chars().take(300).collect(),
        device: if valid_device(device) { device.to_ascii_lowercase() } else { String::new() },
    }
}

pub(crate) fn client_of(request: &axum::extract::Request) -> ClientInfo {
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    client_from(peer, request.headers())
}

fn parse_net(entry: &str) -> Option<(IpAddr, u8)> {
    let entry = entry.trim();
    let (addr, len) = match entry.split_once('/') {
        Some((addr, len)) => (addr.parse::<IpAddr>().ok()?, Some(len.parse::<u8>().ok()?)),
        None => (entry.parse::<IpAddr>().ok()?, None),
    };
    let addr = addr.to_canonical();
    let max = if addr.is_ipv4() { 32 } else { 128 };
    let len = len.unwrap_or(max);
    (len <= max).then_some((addr, len))
}

fn in_net(ip: IpAddr, net: IpAddr, len: u8) -> bool {
    match (ip.to_canonical(), net.to_canonical()) {
        (IpAddr::V4(a), IpAddr::V4(b)) if len <= 32 => {
            let mask = u32::MAX.checked_shl(32 - u32::from(len)).unwrap_or(0);
            u32::from(a) & mask == u32::from(b) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(b)) if len <= 128 => {
            let mask = u128::MAX.checked_shl(128 - u32::from(len)).unwrap_or(0);
            u128::from(a) & mask == u128::from(b) & mask
        }
        _ => false,
    }
}

/// IPv4 按 /24、IPv6 按 /64 算同一网段。
fn same_subnet(a: IpAddr, b: IpAddr) -> bool {
    let a = a.to_canonical();
    in_net(b, a, if a.is_ipv4() { 24 } else { 64 })
}

fn ip_allowed(policy: &LoginPolicy, ip: IpAddr) -> bool {
    policy.ip_allow.is_empty()
        || policy.ip_allow.iter().filter_map(|e| parse_net(e)).any(|(net, len)| in_net(ip, net, len))
}

fn device_allowed(policy: &LoginPolicy, device: &str) -> bool {
    policy.device_allow.is_empty() || policy.device_allow.iter().any(|d| d == device)
}

/// 校验并规范化管理员提交的策略。
pub(crate) fn normalize(mut policy: LoginPolicy) -> Result<LoginPolicy, String> {
    let mut bind = Vec::new();
    for kind in policy.bind {
        if !bind.contains(&kind) {
            bind.push(kind);
        }
    }
    policy.bind = bind;
    policy.ip_allow = policy.ip_allow.iter().map(|e| e.trim().to_owned()).filter(|e| !e.is_empty()).collect();
    policy.device_allow = policy.device_allow.iter().map(|e| e.trim().to_ascii_lowercase()).filter(|e| !e.is_empty()).collect();
    if policy.ip_allow.len() > MAX_ENTRIES || policy.device_allow.len() > MAX_ENTRIES {
        return Err(format!("白名单每类最多 {MAX_ENTRIES} 条"));
    }
    if let Some(bad) = policy.ip_allow.iter().find(|e| parse_net(e).is_none()) {
        return Err(format!("IP 白名单里的「{bad}」不是合法的 IP 或 CIDR"));
    }
    if let Some(bad) = policy.device_allow.iter().find(|e| !valid_device(e)) {
        return Err(format!("设备白名单里的「{bad}」不是合法的设备指纹"));
    }
    policy.ip_allow.dedup();
    policy.device_allow.dedup();
    Ok(policy)
}

/// 登录时：IP 和设备都要在白名单里（白名单为空则不限）。
pub(crate) fn check_login(policy: &LoginPolicy, client: &ClientInfo) -> Result<(), String> {
    if !ip_allowed(policy, client.ip) {
        return Err(format!("当前 IP {} 不在这个账号的登录白名单里，请联系管理员", client.ip));
    }
    if !device_allowed(policy, &client.device) {
        let shown = if client.device.is_empty() { "未知设备".to_owned() } else { client.device.clone() };
        return Err(format!("这台设备（{shown}）不在这个账号的设备白名单里，请把设备指纹发给管理员"));
    }
    Ok(())
}

/// 每个请求：白名单仍然成立，且绑定项与登录时一致。`with_device` 为 false 时跳过设备相关检查
/// （终端 iframe 和 WebSocket 由浏览器直接发起，带不上自定义头）。
pub(crate) fn check_request(
    policy: &LoginPolicy,
    origin: &ClientInfo,
    now: &ClientInfo,
    with_device: bool,
) -> Result<(), String> {
    if !ip_allowed(policy, now.ip) {
        return Err(format!("IP {} 不在登录白名单里，已退出", now.ip));
    }
    if with_device && !device_allowed(policy, &now.device) {
        return Err("这台设备不在设备白名单里，已退出".to_owned());
    }
    let changed = policy.bind.iter().copied().find(|kind| match kind {
        BindKind::Ip => origin.ip != now.ip,
        BindKind::Subnet => !same_subnet(origin.ip, now.ip),
        BindKind::Agent => origin.agent != now.agent,
        BindKind::Device => with_device && origin.device != now.device,
    });
    match changed {
        Some(kind) => Err(format!("登录环境变了（{}与登录时不同），为安全起见已退出，请重新登录", kind.label())),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(ip: &str, agent: &str, device: &str) -> ClientInfo {
        ClientInfo { ip: ip.parse().unwrap(), agent: agent.to_owned(), device: device.to_owned() }
    }

    #[test]
    fn cidr_matching() {
        let (net, len) = parse_net("10.1.0.0/16").unwrap();
        assert!(in_net("10.1.200.3".parse().unwrap(), net, len));
        assert!(!in_net("10.2.0.1".parse().unwrap(), net, len));
        let (net, len) = parse_net("0.0.0.0/0").unwrap();
        assert!(in_net("8.8.8.8".parse().unwrap(), net, len));
        assert!(in_net("::ffff:10.1.2.3".parse().unwrap(), "10.1.2.0".parse().unwrap(), 24));
        assert!(parse_net("10.0.0.0/33").is_none());
        assert!(parse_net("not-an-ip").is_none());
        assert!(same_subnet("192.168.3.4".parse().unwrap(), "192.168.3.200".parse().unwrap()));
        assert!(!same_subnet("192.168.3.4".parse().unwrap(), "192.168.4.4".parse().unwrap()));
    }

    #[test]
    fn whitelist_login() {
        let mut policy = LoginPolicy { ip_allow: vec!["10.0.0.0/8".into()], ..Default::default() };
        assert!(check_login(&policy, &client("10.9.9.9", "ua", "abcdef12")).is_ok());
        assert!(check_login(&policy, &client("11.0.0.1", "ua", "abcdef12")).is_err());
        policy.device_allow = vec!["abcdef12".into()];
        assert!(check_login(&policy, &client("10.0.0.1", "ua", "abcdef12")).is_ok());
        assert!(check_login(&policy, &client("10.0.0.1", "ua", "ffffffff")).is_err());
    }

    #[test]
    fn binding_detects_changes() {
        let start = client("1.2.3.4", "Chrome", "abcdef12");
        let policy = LoginPolicy { bind: vec![BindKind::Subnet, BindKind::Agent, BindKind::Device], ..Default::default() };
        assert!(check_request(&policy, &start, &client("1.2.3.99", "Chrome", "abcdef12"), true).is_ok());
        assert!(check_request(&policy, &start, &client("1.2.4.4", "Chrome", "abcdef12"), true).is_err());
        assert!(check_request(&policy, &start, &client("1.2.3.4", "Firefox", "abcdef12"), true).is_err());
        assert!(check_request(&policy, &start, &client("1.2.3.4", "Chrome", "00000000"), true).is_err());
        // 终端 iframe 带不了设备头：跳过设备比较
        assert!(check_request(&policy, &start, &client("1.2.3.4", "Chrome", ""), false).is_ok());
        let strict = LoginPolicy { bind: vec![BindKind::Ip], ..Default::default() };
        assert!(check_request(&strict, &start, &client("1.2.3.5", "Chrome", "abcdef12"), true).is_err());
    }

    #[test]
    fn normalize_rejects_garbage() {
        let ok = normalize(LoginPolicy {
            bind: vec![BindKind::Ip, BindKind::Ip],
            ip_allow: vec![" 10.0.0.1 ".into(), "".into()],
            device_allow: vec!["ABCDEF12".into()],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(ok.bind, vec![BindKind::Ip]);
        assert_eq!(ok.ip_allow, vec!["10.0.0.1".to_owned()]);
        assert_eq!(ok.device_allow, vec!["abcdef12".to_owned()]);
        assert!(normalize(LoginPolicy { ip_allow: vec!["10.0.0.0/40".into()], ..Default::default() }).is_err());
        assert!(normalize(LoginPolicy { device_allow: vec!["x".into()], ..Default::default() }).is_err());
    }
}
