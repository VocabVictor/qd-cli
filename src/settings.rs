use crate::*;

pub(crate) fn config_command(mut config: Config, args: &mut Args, compact: bool) -> Result<()> {
    match args.pop().as_deref() {
        Some("show") => {
            args.ensure_empty()?;
            print_json(&serde_json::to_value(config)?, compact)
        }
        Some("set") => {
            let key = args.pop().context("config set 缺少 KEY")?;
            let value = args.pop().context("config set 缺少 VALUE")?;
            args.ensure_empty()?;
            config.set(&key, &value)?;
            print_json(&json!({"ok": true, "key": key}), compact)
        }
        _ => bail!("用法: qd config show | qd config set KEY VALUE"),
    }
}
