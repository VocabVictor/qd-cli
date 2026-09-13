use crate::*;

pub(crate) type TerminalSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// 连接平台网页终端 WebSocket 并完成初始化（TERM、工作目录、终端尺寸）。
pub(crate) async fn connect_web_terminal(
    api: &ApiClient,
    terminal_url: &str,
) -> Result<TerminalSocket> {
    let page_url = reqwest::Url::parse(terminal_url).context("网页终端地址无效")?;
    let parameter = |name: &str| {
        page_url
            .query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    let namespace = parameter("namespace").context("网页终端地址缺少 namespace")?;
    let pod = parameter("pod").context("网页终端地址缺少 pod")?;
    let container = parameter("container").context("网页终端地址缺少 container")?;
    let token = parameter("token").context("网页终端地址缺少临时 token")?;

    let mut websocket_url = page_url.clone();
    websocket_url
        .set_scheme(if page_url.scheme() == "https" {
            "wss"
        } else {
            "ws"
        })
        .map_err(|_| anyhow::anyhow!("无法转换网页终端 WebSocket 协议"))?;
    websocket_url.set_query(None);
    websocket_url.set_fragment(None);
    {
        let mut segments = websocket_url
            .path_segments_mut()
            .map_err(|_| anyhow::anyhow!("网页终端地址不能构造 WebSocket 路径"))?;
        segments
            .clear()
            .push("ws")
            .push(&namespace)
            .push(&pod)
            .push(&container)
            .push(&token)
            .push("webshell");
    }

    let mut request = websocket_url
        .as_str()
        .into_client_request()
        .context("构造网页终端 WebSocket 请求失败")?;
    request.headers_mut().insert(
        ORIGIN,
        HeaderValue::from_str(&page_url.origin().ascii_serialization())
            .context("网页终端 Origin 无效")?,
    );
    let connector = if websocket_url.scheme() == "wss" && api.insecure_tls() {
        let tls = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .context("创建网页终端 TLS 配置失败")?;
        Some(Connector::NativeTls(tls))
    } else {
        None
    };
    let (mut websocket, _) = connect_async_tls_with_config(request, None, false, connector)
        .await
        .context("连接网页终端 WebSocket 失败")?;

    send_terminal_input(
        &mut websocket,
        "export TERM=xterm; cd /gemini/code 2>/dev/null || true\r",
    )
    .await?;
    send_terminal_resize(&mut websocket, 160, 40).await?;
    Ok(websocket)
}

pub(crate) async fn dev_web_terminal(
    api: &ApiClient,
    terminal_url: &str,
    remote_command: &[String],
) -> Result<()> {
    let mut websocket = connect_web_terminal(api, terminal_url).await?;

    if remote_command.is_empty() {
        let (input_tx, mut input_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
        std::thread::spawn(move || {
            let mut input = io::stdin();
            let mut buffer = [0u8; 4096];
            loop {
                match input.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(size) => {
                        if input_tx.blocking_send(buffer[..size].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        loop {
            tokio::select! {
                incoming = websocket.next() => {
                    match incoming {
                        Some(Ok(Message::Ping(payload))) => websocket.send(Message::Pong(payload)).await?,
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Ok(message)) => {
                            if let Some(output) = terminal_output(message) {
                                print!("{output}");
                                io::stdout().flush()?;
                            }
                        }
                        Some(Err(error)) => return Err(error).context("读取网页终端失败"),
                    }
                }
                input = input_rx.recv() => {
                    if let Some(input) = input {
                        let input = String::from_utf8_lossy(&input)
                            .replace("\r\n", "\r")
                            .replace('\n', "\r");
                        send_terminal_input(&mut websocket, &input).await?;
                    } else {
                        loop {
                            match tokio::time::timeout(
                                Duration::from_millis(300),
                                websocket.next(),
                            )
                            .await
                            {
                                Ok(Some(Ok(Message::Ping(payload)))) => {
                                    websocket.send(Message::Pong(payload)).await?;
                                }
                                Ok(Some(Ok(message))) => {
                                    if let Some(output) = terminal_output(message) {
                                        print!("{output}");
                                        io::stdout().flush()?;
                                    }
                                }
                                Ok(Some(Err(error))) => {
                                    return Err(error).context("读取网页终端失败");
                                }
                                Ok(None) | Err(_) => break,
                            }
                        }
                        let _ = websocket.close(None).await;
                        return Ok(());
                    }
                }
            }
        }
        return Ok(());
    }

    let (visible, exit_code) = run_web_terminal_command(&mut websocket, remote_command).await?;
    if !visible.is_empty() {
        println!("{visible}");
    }
    if exit_code != 0 {
        bail!("远程命令失败，退出码 {exit_code}");
    }
    Ok(())
}

/// 在已连接的网页终端里执行一条命令，捕获输出与退出码（不打印、不因非零退出报错）。
pub(crate) async fn run_web_terminal_command(
    websocket: &mut TerminalSocket,
    remote_command: &[String],
) -> Result<(String, i32)> {
    send_terminal_input(websocket, "stty -echo 2>/dev/null\r").await?;
    drain_terminal(websocket, Duration::from_millis(250)).await?;
    let nonce = rand::random::<u64>();
    let marker = format!("__QD_EXIT_{nonce}:");
    let command = remote_command
        .iter()
        .map(|argument| shell_quote(argument))
        .collect::<Vec<_>>()
        .join(" ");
    let wrapped = format!(
        "{{ {command}; }}; __qd_ec=$?; printf '\\n__QD_EXIT_%s:%d\\n' '{nonce}' \"$__qd_ec\"; stty echo 2>/dev/null\r"
    );
    send_terminal_input(websocket, &wrapped).await?;

    let mut output = String::new();
    loop {
        match websocket.next().await {
            Some(Ok(Message::Ping(payload))) => websocket.send(Message::Pong(payload)).await?,
            Some(Ok(Message::Close(_))) | None => {
                bail!("网页终端在命令完成前断开");
            }
            Some(Ok(message)) => {
                if let Some(chunk) = terminal_output(message) {
                    output.push_str(&chunk);
                    let cleaned = strip_terminal_control(&output);
                    if let Some(marker_index) = cleaned.find(&marker) {
                        let visible = cleaned[..marker_index]
                            .trim_matches(['\r', '\n', ' '])
                            .to_owned();
                        let exit_text = &cleaned[marker_index + marker.len()..];
                        let exit_code = exit_text
                            .chars()
                            .take_while(|value| value.is_ascii_digit() || *value == '-')
                            .collect::<String>()
                            .parse::<i32>()
                            .context("网页终端返回了无效退出码")?;
                        let _ = websocket.close(None).await;
                        return Ok((visible, exit_code));
                    }
                }
            }
            Some(Err(error)) => return Err(error).context("读取网页终端失败"),
        }
    }
}

pub(crate) async fn send_terminal_input<S>(websocket: &mut S, data: &str) -> Result<()>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    websocket
        .send(Message::Text(
            json!({"operation": "stdin", "data": data})
                .to_string()
                .into(),
        ))
        .await
        .context("向网页终端发送输入失败")
}

pub(crate) async fn send_terminal_resize<S>(websocket: &mut S, cols: u16, rows: u16) -> Result<()>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    websocket
        .send(Message::Text(
            json!({"operation": "resize", "cols": cols, "rows": rows})
                .to_string()
                .into(),
        ))
        .await
        .context("设置网页终端尺寸失败")
}

pub(crate) async fn drain_terminal<S>(websocket: &mut S, idle: Duration) -> Result<()>
where
    S: futures_util::Stream<
            Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
        > + futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    loop {
        match tokio::time::timeout(idle, websocket.next()).await {
            Err(_) => return Ok(()),
            Ok(Some(Ok(Message::Ping(payload)))) => websocket.send(Message::Pong(payload)).await?,
            Ok(Some(Ok(Message::Close(_))) | None) => bail!("网页终端已断开"),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(error))) => return Err(error).context("读取网页终端失败"),
        }
    }
}

