use crate::*;

pub(crate) const HELP: &str = r#"qd - GPU 平台的低内存、高并发命令行客户端

用法:
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
  --concurrency N      临时覆盖并发数（默认 32）
  --secure-tls         校验证书
  --insecure-tls       忽略证书错误（当前内网部署默认）
  --compact            输出单行 JSON
  --full               show/group 输出平台完整响应；默认只输出低 Token 摘要
  -h, --help           显示帮助
  -V, --version        显示版本

安全:
  login 默认隐藏输入密码并用 Windows DPAPI 保存自动登录凭据；可用 --no-remember 禁用。
  Token 和凭据不会出现在命令行、可读配置或输出中。
"#;

pub(crate) async fn run() -> Result<()> {
    let mut args = Args::new(env::args().skip(1));
    if args.is_empty() || args.take_flag_any(&["-h", "--help"]) {
        print!("{HELP}");
        return Ok(());
    }
    if args.take_flag_any(&["-V", "--version"]) {
        println!("qd {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let compact = args.take_flag("--compact");
    let mut config = Config::load()?;
    if let Some(value) = args.take_option("--base-url")? {
        config.base_url = value.trim_end_matches('/').to_owned();
    }
    if let Some(value) = args.take_option("--space-id")? {
        config.space_id = Some(value);
    }
    if let Some(value) = args.take_option("--concurrency")? {
        config.concurrency = parse_usize("concurrency", &value, 1, 1024)?;
    }
    if args.take_flag("--secure-tls") {
        config.insecure_tls = false;
    }
    if args.take_flag("--insecure-tls") {
        config.insecure_tls = true;
    }

    let command = args.pop().context("缺少命令")?;
    match command.as_str() {
        "login" => login(&config, &mut args, compact).await,
        "logout" => logout(compact),
        "whoami" => {
            let api = ApiClient::from_saved_session(config)?;
            let value = api
                .get(Service::Auth, "/user/baseInfo", empty_object())
                .await?;
            print_json(&value, compact)
        }
        "keepalive" => keepalive_command(config, &mut args, compact).await,
        "idle" => {
            args.push_front("idle");
            let api = ApiClient::from_saved_session(config.clone())?;
            resource_command(&api, &config, &mut args, compact).await
        }
        "config" => config_command(config, &mut args, compact),
        "project" => {
            let api = ApiClient::from_saved_session(config.clone())?;
            project_command(&api, &config, &mut args, compact).await
        }
        "job" => {
            if args.peek() == Some("template") {
                args.pop();
                args.ensure_empty()?;
                print_json(&job_template(&config), compact)
            } else {
                let api = ApiClient::from_saved_session(config.clone())?;
                job_command(&api, &config, &mut args, compact).await
            }
        }
        "dev" => {
            if args.peek() == Some("template") {
                args.pop();
                args.ensure_empty()?;
                print_json(&dev_template(&config), compact)
            } else {
                let api = ApiClient::from_saved_session(config.clone())?;
                dev_command(&api, &config, &mut args, compact).await
            }
        }
        "fs" => {
            let api = ApiClient::from_saved_session(config.clone())?;
            fs_command(&api, &config, &mut args, compact).await
        }
        "resource" => {
            let api = ApiClient::from_saved_session(config.clone())?;
            resource_command(&api, &config, &mut args, compact).await
        }
        "image" => {
            let api = ApiClient::from_saved_session(config.clone())?;
            image_command(&api, &config, &mut args, compact).await
        }
        "web" => crate::web::web_command(config, &mut args, compact).await,
        "api" => {
            let api = ApiClient::from_saved_session(config)?;
            raw_api_command(&api, &mut args, compact).await
        }
        "bench" => {
            let api = ApiClient::from_saved_session(config.clone())?;
            bench_command(&api, &config, &mut args, compact).await
        }
        other => bail!("未知命令: {other}\n\n{HELP}"),
    }
}
