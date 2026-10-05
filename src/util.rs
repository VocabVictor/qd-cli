use crate::*;

pub(crate) fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(crate) fn scalar_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(value_string)
}

pub(crate) fn value_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
}

pub(crate) fn value_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

/// Git Bash/MSYS 安装根（如 `E:/Git`、`C:/msys64`）。bash 启动原生 exe 时会把以 /
/// 开头的参数改写成 "<安装根>/..."，而其 usr/bin、mingw64/bin 目录必然出现在
/// 转换后的 PATH 里，据此反推安装根。
pub(crate) fn msys_install_root() -> Option<String> {
    let path = std::env::var_os("PATH")?;
    msys_root_from_entries(std::env::split_paths(&path).map(|entry| entry.to_string_lossy().into_owned()))
}

pub(crate) fn msys_root_from_entries<I: IntoIterator<Item = String>>(entries: I) -> Option<String> {
    entries.into_iter().find_map(|entry| {
        let lower = entry.to_ascii_lowercase();
        ["\\usr\\bin", "/usr/bin", "\\mingw64\\bin", "/mingw64/bin"]
            .iter()
            .filter_map(|marker| lower.find(marker))
            .filter(|index| *index > 0)
            .min()
            .map(|index| entry[..index].trim_end_matches(['\\', '/']).to_string())
    })
}

/// 还原“远端语义”的参数（REST 路由、pod 内路径）。这类参数永远不可能是本地
/// Windows 路径，其中可确定性识别的改写形态直接修复，不必要求调用方设置
/// MSYS2_ARG_CONV_EXCL：
/// - "//job/..."（MSYS 约定的免转换写法）收敛成 "/job/..."；
/// - 已被改写成 "<安装根>/gfs/..." 的，剥掉安装根还原；
/// - "/tmp" 被改写成本地 %TEMP%，映射回 "/tmp"。
/// 其余形态（自定义 fstab 挂载等）无法确定性识别，原样返回交由调用方校验报错。
pub(crate) fn un_mangle_remote_path(raw: &str) -> String {
    un_mangle_remote_path_with(raw, msys_install_root().as_deref())
}

pub(crate) fn un_mangle_remote_path_with(raw: &str, root: Option<&str>) -> String {
    let mut path = raw;
    while path.starts_with("//") {
        path = &path[1..];
    }
    if path.starts_with('/') {
        return path.to_owned();
    }
    let Some(root) = root else {
        return path.to_owned();
    };
    let root = root.trim_end_matches(['\\', '/']).replace('\\', "/");
    if path.len() > root.len()
        && let Some(prefix) = path.get(..root.len())
        && prefix.eq_ignore_ascii_case(&root)
        && path.as_bytes()[root.len()] == b'/'
    {
        return path[root.len()..].to_owned();
    }
    for variable in ["TEMP", "TMP"] {
        let Some(temp) = std::env::var_os(variable) else {
            continue;
        };
        let temp = temp.to_string_lossy().replace('\\', "/");
        let temp = temp.trim_end_matches('/');
        if path.eq_ignore_ascii_case(temp) {
            return "/tmp".to_owned();
        }
        if path.len() > temp.len()
            && let Some(prefix) = path.get(..temp.len())
            && prefix.eq_ignore_ascii_case(temp)
            && path.as_bytes()[temp.len()] == b'/'
        {
            return format!("/tmp{}", &path[temp.len()..]);
        }
    }
    path.to_owned()
}
