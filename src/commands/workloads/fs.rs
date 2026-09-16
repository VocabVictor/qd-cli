use crate::*;

/// 远端路径必须是绝对 Unix 路径。Git Bash/MSYS 会把 `/gfs` 改写成
/// `C:/Program Files/Git/gfs`，命令到了 pod 里必然找不到，且报错完全不指向真因。
fn check_remote_path(path: &str) -> Result<()> {
    let mangled = path.len() >= 2
        && path.as_bytes()[0].is_ascii_alphabetic()
        && path.as_bytes()[1] == b':';
    if mangled {
        bail!(
            "远端路径被本地 shell 改写成了 Windows 路径: {path}\n\
             Git Bash/MSYS 会转换以 / 开头的参数。请先执行:\n\
             export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'\n\
             或改用 PowerShell 运行同一条命令。"
        );
    }
    if !path.starts_with('/') {
        bail!("远端路径必须是绝对路径（以 / 开头），收到: {path}");
    }
    Ok(())
}

/// 执行目标：优先显式指定，否则自动挑一个 RUNNING 的开发机，再退回 RUNNING 的作业。
enum Target {
    Dev(String),
    Job(String, usize),
}

async fn resolve_target(api: &ApiClient, config: &Config, args: &mut Args) -> Result<Target> {
    if let Some(id) = args.take_option("--dev")? {
        return Ok(Target::Dev(id));
    }
    if let Some(id) = args.take_option("--job")? {
        let instance = option_u64(args, "--instance", 0, 0, 64)? as usize;
        return Ok(Target::Job(id, instance));
    }
    let devs = list_scoped_dev_environments(api, "mine").await?;
    if let Some(id) = first_running_id(&devs, "/data/devList", &["jobenvId"]) {
        return Ok(Target::Dev(id));
    }
    if let Ok(jobs) =
        list_scoped_jobs(api, config, "mine", &json!({"pageNum": 1, "pageSize": 50}), None).await
        && let Some(id) = first_running_id(&jobs, "/data/jobList", &["id", "jobId"])
    {
        return Ok(Target::Job(id, 0));
    }
    bail!(
        "没有可用的执行目标：既没有 RUNNING 的开发机，也没有 RUNNING 的作业。\n\
         可以先 `qd dev start <JOBENV_ID>`，或用 --dev / --job 显式指定。"
    )
}

fn first_running_id(response: &Value, pointer: &str, id_keys: &[&str]) -> Option<String> {
    let entries = response.pointer(pointer)?.as_array()?;
    for entry in entries {
        let status = pointer_text(entry, &["/jobenvStatus", "/status"]);
        if !status.starts_with("RUNNING") {
            continue;
        }
        for key in id_keys {
            if let Some(value) = entry.get(*key) {
                if let Some(text) = value.as_str().filter(|value| !value.is_empty()) {
                    return Some(text.to_owned());
                }
                if let Some(number) = value.as_u64().filter(|number| *number > 0) {
                    return Some(number.to_string());
                }
            }
        }
    }
    None
}

async fn run(api: &ApiClient, target: Target, script: String) -> Result<()> {
    let command = vec!["sh".to_owned(), "-c".to_owned(), script];
    match target {
        Target::Dev(id) => dev_terminal_transport(api, &id, &command).await,
        Target::Job(id, instance) => job_exec(api, &id, instance, &command).await,
    }
}

pub(crate) async fn fs_command(
    api: &ApiClient,
    config: &Config,
    args: &mut Args,
    _compact: bool,
) -> Result<()> {
    let sub = args.pop().context("用法: qd fs ls|cat|find|du|stat PATH")?;
    let target = resolve_target(api, config, args).await?;
    let path = args.pop().context("缺少 PATH")?;
    check_remote_path(&path)?;
    let quoted = shell_quote(&path);

    let script = match sub.as_str() {
        "ls" => {
            let long = args.take_flag("--long");
            let all = args.take_flag("--all");
            args.ensure_empty()?;
            let flags = match (long, all) {
                (true, true) => "-la --time-style=long-iso",
                (true, false) => "-l --time-style=long-iso",
                (false, true) => "-A",
                (false, false) => "-1",
            };
            format!("ls {flags} -- {quoted}")
        }
        "cat" => {
            let lines = option_u64(args, "--lines", 200, 1, 100_000)?;
            args.ensure_empty()?;
            format!("head -n {lines} -- {quoted}")
        }
        "find" => {
            let name = args.take_option("--name")?;
            let depth = option_u64(args, "--max-depth", 3, 1, 32)?;
            let limit = option_u64(args, "--limit", 500, 1, 100_000)?;
            args.ensure_empty()?;
            let filter = match name {
                Some(pattern) => format!(" -name {}", shell_quote(&pattern)),
                None => String::new(),
            };
            format!("find {quoted} -maxdepth {depth}{filter} 2>/dev/null | head -n {limit}")
        }
        "du" => {
            let depth = option_u64(args, "--depth", 1, 0, 16)?;
            args.ensure_empty()?;
            format!("du -h --max-depth={depth} -- {quoted} 2>/dev/null | sort -h")
        }
        "stat" => {
            args.ensure_empty()?;
            format!("stat -- {quoted}")
        }
        other => bail!("未知子命令: {other}\n用法: qd fs ls|cat|find|du|stat PATH"),
    };
    run(api, target, script).await
}
