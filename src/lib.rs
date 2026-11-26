//! qd（qudongctl）的核心库：GPU 训练平台 Web API 的命令行封装。
//!
//! 分层（上层可依赖下层，反之不行）：
//! - 基础层：`config`、`session`、`credentials`、`api`、`util`
//! - 命令层：`commands`（account / catalog / system / workloads）
//! - 呈现层：`output`、`terminal`、`web`
//! - 入口：`cli`（参数与帮助）、`app`（全局选项与分发）

mod api;
mod app;
mod cli;
mod commands;
mod config;
mod credentials;
mod output;
mod prelude;
mod session;
mod terminal;
mod util;
mod web;

pub use app::run;

/// Windows 控制台默认使用旧代码页，切到 UTF-8 才能正确显示中文；其它平台无操作。
#[cfg(windows)]
pub fn configure_console() {
    use windows_sys::Win32::System::Console::{SetConsoleCP, SetConsoleOutputCP};

    const CP_UTF8: u32 = 65001;
    // These calls fail harmlessly when stdout/stdin are redirected instead of
    // attached to a console. In an interactive Windows PowerShell console they
    // prevent UTF-8 Chinese text from being decoded with the legacy code page.
    unsafe {
        let _ = SetConsoleOutputCP(CP_UTF8);
        let _ = SetConsoleCP(CP_UTF8);
    }
}

#[cfg(not(windows))]
pub fn configure_console() {}

#[cfg(test)]
mod tests;
