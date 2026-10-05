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
    let instance = data.pointer("/devJobDetail/taskroleList/0/instanceList/0");
    let text_field = |name: &str| {
        instance
            .and_then(|value| value.get(name))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty() && *value != "-")
    };
    // 非交互执行与作业 exec 同走网页终端 websocket,不依赖本地 SSH 配置;
    // 交互 shell 仍优先 SSH(本地 PTY 体验更好),websocket 作后备。
    let prefer_websocket = !remote_command.is_empty();
    let terminal_url = text_field("terminalUrl");
    let ssh_connection = text_field("sshConnection");
    if prefer_websocket && let Some(terminal_url) = terminal_url {
        return dev_web_terminal(api, &terminal_url, remote_command).await;
    }
    if let Some(connection) = ssh_connection {
        return dev_system_ssh(&connection, remote_command);
    }
    if let Some(terminal_url) = terminal_url {
        return dev_web_terminal(api, &terminal_url, remote_command).await;
    }
    bail!("平台既未返回网页终端地址，也未返回 SSH 连接信息")
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
            // 平台 SSH 网关（JumpServer）只接受 ssh-rsa（SHA-1）签名，
            // OpenSSH 默认算法集会被拒，这里显式放行。
            "-o",
            "PubkeyAcceptedAlgorithms=+ssh-rsa",
        ])
        .args(&parts)
        .args(remote_command)
        .status()
        .context("启动系统 SSH 失败")?;
    if !status.success() {
        bail!(
            "SSH 命令失败，退出码 {}。网关可能要求指定密钥：请在 ~/.ssh/config 为该主机 \
             配置 IdentityFile 与 PubkeyAcceptedAlgorithms +ssh-rsa，或改用 qd dev shell。",
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}
