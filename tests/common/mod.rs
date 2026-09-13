#![allow(dead_code)]

mod server;

use server::handle_request;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rand::rngs::OsRng;
use rsa::{
    Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
    pkcs8::{EncodePublicKey, LineEnding},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};
use tempfile::TempDir;
use tiny_http::{Header, Request, Response, Server, StatusCode};

#[derive(Default)]
pub struct Observed {
    pub login_username: Mutex<Option<String>>,
    pub login_password: Mutex<Option<String>>,
    pub login_calls: AtomicUsize,
    pub refreshes: AtomicUsize,
    pub fail_refresh: AtomicBool,
    pub whoami_calls: AtomicUsize,
    pub project_list_calls: AtomicUsize,
    pub cancel_calls: AtomicUsize,
    pub job_submit_calls: AtomicUsize,
    pub resource_stock_calls: AtomicUsize,
}

pub struct MockServer {
    pub base_url: String,
    pub observed: Arc<Observed>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl MockServer {
    pub fn start(delay_ms: u64) -> Self {
        let server = Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let private_key = Arc::new(RsaPrivateKey::new(&mut OsRng, 2048).unwrap());
        let public_pem = RsaPublicKey::from(private_key.as_ref())
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        let observed = Arc::new(Observed::default());
        let stop = Arc::new(AtomicBool::new(false));
        let thread_observed = observed.clone();
        let thread_stop = stop.clone();
        let thread = thread::spawn(move || {
            let mut workers = Vec::new();
            while !thread_stop.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(20)) {
                    Ok(Some(request)) => {
                        let private_key = private_key.clone();
                        let public_pem = public_pem.clone();
                        let observed = thread_observed.clone();
                        workers.push(thread::spawn(move || {
                            handle_request(request, &private_key, &public_pem, &observed, delay_ms);
                        }));
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
            for worker in workers {
                let _ = worker.join();
            }
        });
        Self {
            base_url: format!("http://{address}"),
            observed,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn isolated_environment(base_url: &str, concurrency: usize) -> TempDir {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("appdata").join("qd");
    let state_dir = temp.path().join("localappdata");
    fs::create_dir_all(&config_dir).unwrap();
    fs::create_dir_all(&state_dir).unwrap();
    let config = json!({
        "base_url": base_url,
        "space_id": "testspace001",
        "concurrency": concurrency,
        "connect_timeout_secs": 2,
        "request_timeout_secs": 10,
        "insecure_tls": false
    });
    fs::write(
        config_dir.join("config.json"),
        serde_json::to_vec_pretty(&config).unwrap(),
    )
    .unwrap();
    temp
}

pub fn run_qd(temp: &TempDir, args: &[&str], stdin: Option<&str>) -> Output {
    let mut command = qd_command(temp);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

pub fn qd_command(temp: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_qd"));
    command
        .env("APPDATA", temp.path().join("appdata"))
        .env("LOCALAPPDATA", temp.path().join("localappdata"))
        .env_remove("QD_TOKEN")
        .env_remove("QD_REFRESH_TOKEN")
        .env_remove("QD_USERNAME")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost");
    command
}

pub fn stdout_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "qd failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn session_file(temp: &TempDir) -> std::path::PathBuf {
    temp.path()
        .join("localappdata")
        .join("qd")
        .join("session.bin")
}

pub fn credentials_file(temp: &TempDir) -> std::path::PathBuf {
    temp.path()
        .join("localappdata")
        .join("qd")
        .join("credentials.bin")
}

pub fn assert_file_exists(path: &Path) {
    assert!(path.exists(), "expected {} to exist", path.display());
}
