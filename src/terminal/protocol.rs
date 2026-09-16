use crate::*;

pub(crate) fn terminal_output(message: Message) -> Option<String> {
    let text = match message {
        Message::Text(text) => text.to_string(),
        Message::Binary(bytes) => String::from_utf8(bytes.to_vec()).ok()?,
        _ => return None,
    };
    let value: Value = serde_json::from_str(&text).ok()?;
    (value.get("operation").and_then(Value::as_str) == Some("stdout")).then(|| {
        value
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    })
}

pub(crate) fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"@%_+=:,./-".contains(&byte))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

pub(crate) fn strip_terminal_control(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            if chars.next_if_eq(&'[').is_some() {
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            } else if chars.next_if_eq(&']').is_some() {
                // OSC（如 ESC ] 0 ; 标题 BEL）以 BEL 或 ST(ESC \) 结束；
                // 不吃掉它，终端标题就会混进命令输出里
                while let Some(next) = chars.next() {
                    if next == '\u{0007}' {
                        break;
                    }
                    if next == '\u{1b}' {
                        chars.next_if_eq(&'\\');
                        break;
                    }
                }
            }
            continue;
        }
        if character == '\u{0008}' {
            output.pop();
        } else if character == '\r' {
            output.push('\n');
            chars.next_if_eq(&'\n');
        } else if character == '\n' || character == '\t' || !character.is_control() {
            output.push(character);
        }
    }
    output
}
