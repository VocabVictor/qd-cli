//! 登录/注册失败限速：同一来源 IP 或同一用户名在窗口内失败达到上限即锁到窗口结束。
//! 纯内存，进程重启清零。

use std::collections::HashMap;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_FAILURES: u32 = 10;

#[derive(Default)]
pub(crate) struct AuthLimiter {
    failures: tokio::sync::Mutex<HashMap<String, (u32, Instant)>>,
}

impl AuthLimiter {
    /// 任一 key 已被锁定则返回剩余锁定秒数。
    pub(crate) async fn locked_for(&self, keys: &[&str]) -> Option<u64> {
        let mut map = self.failures.lock().await;
        let now = Instant::now();
        map.retain(|_, (_, start)| now.duration_since(*start) < WINDOW);
        keys.iter()
            .filter_map(|key| map.get(*key))
            .filter(|(count, _)| *count >= MAX_FAILURES)
            .map(|(_, start)| (WINDOW - now.duration_since(*start)).as_secs().max(1))
            .max()
    }

    pub(crate) async fn record_failure(&self, keys: &[&str]) {
        let mut map = self.failures.lock().await;
        let now = Instant::now();
        for key in keys {
            let entry = map.entry((*key).to_owned()).or_insert((0, now));
            if now.duration_since(entry.1) >= WINDOW {
                *entry = (0, now);
            }
            entry.0 += 1;
        }
    }

    pub(crate) async fn clear(&self, key: &str) {
        self.failures.lock().await.remove(key);
    }
}
