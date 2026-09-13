use crate::*;

pub(crate) async fn login(config: &Config, args: &mut Args, compact: bool) -> Result<()> {
    let username = args
        .take_option_any(&["--username", "-u"])?
        .context("login 需要 --username USER")?;
    let mut password = if args.take_flag("--password-stdin") {
        read_stdin_trimmed().context("从标准输入读取密码失败")?
    } else {
        rpassword::prompt_password("密码: ").context("读取密码失败")?
    };
    if password.is_empty() {
        bail!("密码不能为空");
    }
    let ldap = args.take_flag("--ldap");
    let remember = !args.take_flag("--no-remember");
    let captcha_id = args.take_option("--captcha-id")?;
    let captcha_code = args.take_option("--captcha-code")?;
    args.ensure_empty()?;
    let login_result = ApiClient::login(
        config,
        &username,
        &password,
        ldap,
        captcha_id.as_deref(),
        captcha_code.as_deref(),
    )
    .await;
    let session = match login_result {
        Ok(session) => session,
        Err(error) => {
            clear_string(&mut password);
            return Err(error);
        }
    };
    session.save()?;
    if remember {
        SavedCredentials::new(username.clone(), password, ldap).save()?;
    } else {
        clear_string(&mut password);
        SavedCredentials::remove()?;
    }
    print_json(
        &json!({
            "ok": true,
            "username": username,
            "credentialStore": "Windows DPAPI",
            "autoLogin": remember
        }),
        compact,
    )
}

pub(crate) fn logout(compact: bool) -> Result<()> {
    let session_removed = Session::remove()?;
    let credential_removed = SavedCredentials::remove()?;
    print_json(
        &json!({
            "ok": true,
            "sessionRemoved": session_removed,
            "credentialRemoved": credential_removed
        }),
        compact,
    )
}
