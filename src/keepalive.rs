use crate::*;

#[cfg(windows)]
pub(crate) const KEEPALIVE_TASK_NAME: &str = "qd-keepalive";

pub(crate) async fn keepalive_command(
    config: Config,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    match args.pop().as_deref() {
        Some("once") => {
            let quiet = args.take_flag("--quiet");
            args.ensure_empty()?;
            let api = ApiClient::from_saved_session(config)?;
            let value = keepalive_once(&api).await?;
            if quiet {
                Ok(())
            } else {
                print_json(&value, compact)
            }
        }
        Some("install") => {
            let minutes = option_u64(args, "--minutes", 30, 5, 1440)?;
            args.ensure_empty()?;
            print_json(&install_keepalive_task(minutes)?, compact)
        }
        Some("status") => {
            args.ensure_empty()?;
            print_json(&keepalive_task_status()?, compact)
        }
        Some("remove") => {
            args.ensure_empty()?;
            print_json(&remove_keepalive_task()?, compact)
        }
        _ => bail!("用法: qd keepalive once|install|status|remove"),
    }
}

pub(crate) async fn keepalive_once(api: &ApiClient) -> Result<Value> {
    api.refresh_session().await?;
    api.get(Service::Auth, "/user/baseInfo", empty_object())
        .await?;
    Ok(json!({"ok": true, "refreshed": true}))
}

#[cfg(windows)]
pub(crate) fn install_keepalive_task(minutes: u64) -> Result<Value> {
    let executable = env::current_exe().context("无法定位 qd.exe")?;
    let executable = executable.to_str().context("qd.exe 路径不是有效 Unicode")?;
    if executable.contains('"') {
        bail!("qd.exe 路径不能包含双引号");
    }
    let action = format!("\"{executable}\" keepalive once --quiet");
    let output = Command::new("schtasks.exe")
        .args([
            "/Create",
            "/SC",
            "MINUTE",
            "/MO",
            &minutes.to_string(),
            "/TN",
            KEEPALIVE_TASK_NAME,
            "/TR",
            &action,
            "/RL",
            "LIMITED",
            "/F",
        ])
        .output()
        .context("无法调用 Windows 任务计划程序")?;
    if !output.status.success() {
        bail!("创建保活任务失败: {}", process_error(&output));
    }
    Ok(json!({
        "ok": true,
        "installed": true,
        "taskName": KEEPALIVE_TASK_NAME,
        "minutes": minutes,
        "executable": executable
    }))
}

#[cfg(not(windows))]
pub(crate) fn install_keepalive_task(_minutes: u64) -> Result<Value> {
    bail!("自动保活任务目前只支持 Windows")
}

#[cfg(windows)]
pub(crate) fn keepalive_task_status() -> Result<Value> {
    let output = Command::new("schtasks.exe")
        .args(["/Query", "/TN", KEEPALIVE_TASK_NAME])
        .output()
        .context("无法查询 Windows 任务计划程序")?;
    Ok(json!({
        "ok": true,
        "installed": output.status.success(),
        "taskName": KEEPALIVE_TASK_NAME
    }))
}

#[cfg(not(windows))]
pub(crate) fn keepalive_task_status() -> Result<Value> {
    bail!("自动保活任务目前只支持 Windows")
}

#[cfg(windows)]
pub(crate) fn remove_keepalive_task() -> Result<Value> {
    let query = Command::new("schtasks.exe")
        .args(["/Query", "/TN", KEEPALIVE_TASK_NAME])
        .output()
        .context("无法查询 Windows 任务计划程序")?;
    if !query.status.success() {
        return Ok(json!({"ok": true, "removed": false, "taskName": KEEPALIVE_TASK_NAME}));
    }
    let output = Command::new("schtasks.exe")
        .args(["/Delete", "/TN", KEEPALIVE_TASK_NAME, "/F"])
        .output()
        .context("无法调用 Windows 任务计划程序")?;
    if !output.status.success() {
        bail!("删除保活任务失败: {}", process_error(&output));
    }
    Ok(json!({"ok": true, "removed": true, "taskName": KEEPALIVE_TASK_NAME}))
}

#[cfg(not(windows))]
pub(crate) fn remove_keepalive_task() -> Result<Value> {
    bail!("自动保活任务目前只支持 Windows")
}

#[cfg(windows)]
fn process_error(output: &std::process::Output) -> String {
    let bytes = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    String::from_utf8_lossy(bytes).trim().to_owned()
}
