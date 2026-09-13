mod api;
mod app;
mod auth;
mod cli;
mod commands;
mod config;
mod credentials;
mod keepalive;
mod output;
mod resources;
mod session;
mod settings;
mod terminal;
mod util;
mod web;

use anyhow::{Context, Result, bail};

use api::{ApiClient, Service, empty_object};

use config::Config;

use credentials::{SavedCredentials, clear_string};

use futures_util::{SinkExt, StreamExt};

use reqwest::Method;

use serde_json::{Value, json};

use session::Session;

use std::{
    collections::VecDeque,
    env, fs,
    io::{self, Read, Write},
    path::Path,
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tokio::task::JoinSet;

use tokio_tungstenite::{
    Connector, connect_async_tls_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderValue, header::ORIGIN},
    },
};

pub(crate) use app::*;
pub(crate) use auth::*;
pub(crate) use cli::*;
pub(crate) use commands::*;
pub(crate) use keepalive::*;
pub(crate) use output::*;
pub(crate) use resources::*;
pub(crate) use settings::*;
pub(crate) use terminal::*;
pub(crate) use util::*;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    configure_windows_console_utf8();
    if let Err(error) = run().await {
        eprintln!("错误: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn configure_windows_console_utf8() {
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
fn configure_windows_console_utf8() {}

#[cfg(test)]
mod tests;
