use crate::*;

pub(crate) async fn dev_terminal_transport(
    api: &ApiClient,
    id: &str,
    remote_command: &[String],
) -> Result<()> {
    let path = format!("/jobenv/refresh/{id}");
    let response = api.get(Service::Core, &path, empty_object()).await?;
    let data = response_data(&response);
    let status = pointer_text(data, &["/status"]);
    if status != "RUNNING" {
        bail!("开发环境 {id} 当前状态为 {status}，请先启动");
    }
    if let Some(connection) = data
        .pointer("/devJobDetail/taskroleList/0/instanceList/0/sshConnection")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && *value != "-")
    {
        return dev_system_ssh(connection, remote_command);
    }
    let terminal_url = data
        .pointer("/devJobDetail/taskroleList/0/instanceList/0/terminalUrl")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && *value != "-")
        .context("平台既未返回 SSH 连接，也未返回网页终端地址")?;
    dev_web_terminal(api, terminal_url, remote_command).await
}

pub(crate) fn dev_system_ssh(connection: &str, remote_command: &[String]) -> Result<()> {
    let mut parts = connection
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        bail!("平台返回的 SSH 连接信息为空");
    }
    let program = if parts[0]
        .trim_matches('"')
        .to_ascii_lowercase()
        .ends_with("ssh.exe")
        || parts[0].eq_ignore_ascii_case("ssh")
    {
        parts.remove(0)
    } else {
        "ssh".to_owned()
    };
    let status = Command::new(program)
        .args([
            "-o",
            "ConnectTimeout=10",
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=accept-new",
        ])
        .args(&parts)
        .args(remote_command)
        .status()
        .context("启动系统 SSH 失败")?;
    if !status.success() {
        bail!("SSH 命令失败，退出码 {}", status.code().unwrap_or(-1));
    }
    Ok(())
}
