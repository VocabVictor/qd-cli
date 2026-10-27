//! 本机账号 session：纯内存，`qd web` 进程重启即全部失效，不落盘。
//! 每个 session 记下登录时的来源（IP / 浏览器 / 设备指纹），供 login_policy 做绑定校验；
//! 被踢下线的 token 留一段时间的原因，让前端能告诉用户为什么被退出。

use super::login_policy::ClientInfo;
use crate::*;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub(crate) const SESSION_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// 多久没有请求就不算"在线"：拒绝模式下超过这个时间，别处就可以重新登录。
pub(crate) const IDLE_LIMIT: Duration = Duration::from_secs(30 * 60);
const NOTICE_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Clone)]
pub(crate) struct Session {
    /// 给管理后台用的短 ID（不是 token，泄露了也不能拿来登录）
    pub(crate) id: String,
    pub(crate) user: String,
    pub(crate) client: ClientInfo,
    pub(crate) created: i64,
    pub(crate) seen_unix: i64,
    seen_at: Instant,
    expires: Instant,
}

impl Session {
    pub(crate) fn online(&self) -> bool {
        self.seen_at.elapsed() < IDLE_LIMIT
    }
}

#[derive(Default)]
pub(crate) struct SessionStore {
    live: RwLock<HashMap<String, Session>>,
    notices: RwLock<HashMap<String, (String, Instant)>>,
}

fn random_text(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn unix_now() -> i64 {
    (unix_millis() / 1000) as i64
}

impl SessionStore {
    pub(crate) async fn create(&self, user: &str, client: ClientInfo) -> String {
        let token = random_text(32);
        let now = Instant::now();
        let session = Session {
            id: random_text(9),
            user: user.to_owned(),
            client,
            created: unix_now(),
            seen_unix: unix_now(),
            seen_at: now,
            expires: now + SESSION_TTL,
        };
        self.live.write().await.insert(token.clone(), session);
        token
    }

    /// token 有效则刷新最近活跃时间并返回；顺手清掉过期记录。
    pub(crate) async fn touch(&self, token: &str) -> Option<Session> {
        let mut live = self.live.write().await;
        let now = Instant::now();
        live.retain(|_, s| s.expires > now);
        let session = live.get_mut(token)?;
        session.seen_at = now;
        session.seen_unix = unix_now();
        Some(session.clone())
    }

    async fn note(&self, tokens: Vec<String>, reason: &str) {
        let mut notices = self.notices.write().await;
        notices.retain(|_, (_, at)| at.elapsed() < NOTICE_TTL);
        for token in tokens {
            notices.insert(token, (reason.to_owned(), Instant::now()));
        }
    }

    pub(crate) async fn revoke(&self, token: &str, reason: Option<&str>) {
        let removed = self.live.write().await.remove(token).is_some();
        if let (true, Some(reason)) = (removed, reason) {
            self.note(vec![token.to_owned()], reason).await;
        }
    }

    /// 踢掉某用户的全部 session（`keep` 指定的除外）。
    pub(crate) async fn revoke_user(&self, user: &str, reason: Option<&str>, keep: Option<&str>) {
        let mut gone = Vec::new();
        self.live.write().await.retain(|token, s| {
            let drop = s.user.eq_ignore_ascii_case(user) && Some(token.as_str()) != keep;
            if drop {
                gone.push(token.clone());
            }
            !drop
        });
        if let Some(reason) = reason {
            self.note(gone, reason).await;
        }
    }

    /// 按管理后台的短 ID 踢人，返回被踢的用户名。
    pub(crate) async fn revoke_id(&self, id: &str, reason: &str) -> Option<String> {
        let mut live = self.live.write().await;
        let token = live.iter().find(|(_, s)| s.id == id).map(|(t, _)| t.clone())?;
        let session = live.remove(&token)?;
        drop(live);
        self.note(vec![token], reason).await;
        Some(session.user)
    }

    /// 该用户最近仍在活跃的 session（拒绝模式用）。
    pub(crate) async fn online_of(&self, user: &str) -> Option<Session> {
        let live = self.live.read().await;
        let now = Instant::now();
        live.values()
            .filter(|s| s.user.eq_ignore_ascii_case(user) && s.expires > now && s.online())
            .max_by_key(|s| s.seen_unix)
            .cloned()
    }

    pub(crate) async fn id_of(&self, token: &str) -> Option<String> {
        self.live.read().await.get(token).map(|s| s.id.clone())
    }

    pub(crate) async fn list(&self) -> Vec<Session> {
        let now = Instant::now();
        let mut all: Vec<Session> = self.live.read().await.values().filter(|s| s.expires > now).cloned().collect();
        all.sort_by(|a, b| b.seen_unix.cmp(&a.seen_unix));
        all
    }

    /// 这个 token 为什么失效（被踢 / 环境变化），没有记录则为 None。
    pub(crate) async fn notice(&self, token: &str) -> Option<String> {
        let notices = self.notices.read().await;
        notices
            .get(token)
            .filter(|(_, at)| at.elapsed() < NOTICE_TTL)
            .map(|(reason, _)| reason.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(ip: &str) -> ClientInfo {
        ClientInfo { ip: ip.parse().unwrap(), agent: "ua".into(), device: "abcdef12".into() }
    }

    #[tokio::test]
    async fn revoke_leaves_notice() {
        let store = SessionStore::default();
        let a = store.create("alice", client("1.1.1.1")).await;
        let b = store.create("Alice", client("2.2.2.2")).await;
        let c = store.create("bob", client("3.3.3.3")).await;
        assert!(store.online_of("alice").await.is_some());
        store.revoke_user("alice", Some("在别处登录"), Some(&b)).await;
        assert!(store.touch(&a).await.is_none());
        assert!(store.touch(&b).await.is_some());
        assert!(store.touch(&c).await.is_some());
        assert_eq!(store.notice(&a).await.as_deref(), Some("在别处登录"));
        assert!(store.notice(&b).await.is_none());
        let id = store.list().await.into_iter().find(|s| s.user == "bob").unwrap().id;
        assert_eq!(store.revoke_id(&id, "管理员踢下线").await.as_deref(), Some("bob"));
        assert!(store.touch(&c).await.is_none());
    }
}
