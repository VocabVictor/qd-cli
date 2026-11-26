use crate::prelude::*;
use crate::api::ApiClient;
use crate::api::Service;
use crate::api::empty_object;
use crate::cli::Args;
use crate::cli::HELP;
use crate::cli::help_section;
use crate::cli::parse_usize;
use crate::cli::print_json;
use crate::commands::bench_command;
use crate::commands::config_command;
use crate::commands::dev_command;
use crate::commands::dev_template;
use crate::commands::fs_command;
use crate::commands::image_command;
use crate::commands::job_command;
use crate::commands::job_template;
use crate::commands::keepalive_command;
use crate::commands::login;
use crate::commands::logout;
use crate::commands::project_command;
use crate::commands::raw_api_command;
use crate::commands::resource_command;
use crate::config::Config;
use std::env;

/// 解析全局选项并分发到各子命令。
pub async fn run() -> Result<()> {
    let mut args = Args::new(env::args().skip(1));
    if args.take_flag_any(&["-V", "--version"]) {
        println!("qd {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let Some(command) = args.pop() else {
        print!("{HELP}");
        return Ok(());
    };
    if args.take_flag_any(&["-h", "--help"]) {
        let section = help_section(&command);
        print!("{}", if section.is_empty() { HELP } else { &section });
        return Ok(());
    }

    let compact = args.take_flag("--compact");
    let mut config = Config::load()?;
    if args.take_flag("--verbose") {
        config.verbose = true;
    }
    if let Some(value) = args.take_option("--base-url")? {
        config.base_url = value.trim_end_matches('/').to_owned();
    }
    if let Some(value) = args.take_option("--space-id")? {
        config.space_id = Some(value);
    }
    if let Some(value) = args.take_option("--proxy")? {
        config.set_proxy(&value)?;
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
