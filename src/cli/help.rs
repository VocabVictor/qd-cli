pub(crate) const HELP: &str = r#"qd - GPU 平台的低内存、高并发命令行客户端

用法:
  选项可放在任意位置；`--` 之后的参数原样传给远端命令。
  qd login --username USER [--password-stdin] [--ldap] [--no-remember]
  qd logout
  qd whoami
  qd keepalive once [--quiet]
  qd keepalive install [--minutes N]
  qd keepalive status
  qd keepalive remove
  qd idle [RSGROUP_ID...] [--available-only|--gpu-only] [--concurrency N]
  qd config show
  qd config set KEY VALUE

  qd project list [--scope mine|shared|public|all] [--page N] [--size N]
  qd project show PROJECT_ID [--full]
  qd project create --file project.json

  qd job list [PROJECT_ID] [--scope mine|shared|public|all] [--page N] [--size N] [--status STATUS]
  qd job show JOB_ID [--full]
  qd job submit --file job.json [--wait]
  qd job cancel JOB_ID... [--concurrency N]
  qd job delete JOB_ID... [--concurrency N]
  qd job wait JOB_ID... [--interval SEC] [--timeout SEC]
  qd job template
  qd job logs JOB_ID [--limit N]
  qd job ssh JOB_ID
  qd job exec JOB_ID [--instance N] -- COMMAND [ARG...]

  qd dev list [--scope mine|shared|public|all]
  qd dev show JOBENV_ID [--full]
  qd dev create --file dev.json
  qd dev start JOBENV_ID...
  qd dev stop JOBENV_ID...
  qd dev tools JOBENV_ID [--set jupyterlab,tensorboard|none]
  qd dev ssh-config JOBENV_ID --enable|--disable
  qd dev refresh JOBENV_ID
  qd dev terminal-info JOBENV_ID
  qd dev shell JOBENV_ID
  qd dev exec JOBENV_ID -- COMMAND [ARG...]
  qd dev template

  qd fs ls PATH [--long] [--all] [--dev ID|--job ID [--instance N]]
  qd fs cat PATH [--lines N]
  qd fs find PATH [--name PATTERN] [--max-depth N] [--limit N]
  qd fs du PATH [--depth N]
  qd fs stat PATH
      在 pod 内列举/查看文件；不指定目标时自动选一个 RUNNING 的开发机，
      没有则退回 RUNNING 的作业。

  qd resource groups [--page N] [--size N]
  qd resource group RSGROUP_ID [--full]
  qd resource idle [RSGROUP_ID...] [--available-only|--gpu-only] [--concurrency N]
  qd resource nodes [--group ID] [--gpu-type physical|virtual|all] [--gpu-model TEXT]
                    [--min-gpu N] [--min-cpu N] [--min-memory-gb N]
                    [--sort gpu|cpu|memory] [--asc|--desc] [--jobs] [--page N] [--size N]
  qd resource node NODE_OR_IP [--group ID]
  qd image list [--source current|mine] [--keywords TEXT] [--page N] [--size N]
  qd image repositories [--page N] [--size N]

  qd web [--port N] [--open]
  qd api METHOD PATH [--service core|auth|tool|absolute] [--file body.json]
  qd bench PATH [--service core|auth|tool|absolute] [--file query.json]
                   [--requests N] [--warmup N] [--concurrency N]

全局选项（可放在任意位置）:
  --base-url URL       临时覆盖平台地址
  --space-id ID        临时覆盖工作空间 ID
  --proxy URL          显式使用代理；none 为直连（默认忽略系统和环境代理）
  --concurrency N      临时覆盖并发数（默认 32）
  --secure-tls         校验证书
  --insecure-tls       忽略证书错误（当前内网部署默认）
  --compact            输出单行 JSON
  --verbose            在 stderr 打印每个请求的方法、URL、状态与耗时
  --full               show/group 输出平台完整响应；默认只输出低 Token 摘要
  -h, --help           显示帮助
  -V, --version        显示版本

安全:
  login 默认隐藏输入密码并用 Windows DPAPI 保存自动登录凭据；可用 --no-remember 禁用。
  Token 和凭据不会出现在命令行、可读配置或输出中。
"#;

/// 从 HELP 摘出某命令组的段落（按空行分块，块内任一行以 `qd <command>` 起始）；
/// 没有对应段落时返回空串，由调用方回退到完整帮助。
pub(crate) fn help_section(command: &str) -> String {
    let prefix = format!("qd {command} ");
    let bare = format!("qd {command}");
    HELP.split("

")
        .filter(|block| {
            block.lines().any(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with(&prefix) || trimmed == bare
            })
        })
        .collect::<Vec<_>>()
        .join("

")
}
