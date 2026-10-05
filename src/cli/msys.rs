use std::path::Path;
use std::sync::OnceLock;

/// Git Bash/MSYS 在启动原生 exe 前会按它自己的挂载表改写以 / 开头的参数
/// （例如 `/gfs` → `<安装根>/gfs`、`/tmp` → 用户临时目录）。qd 的 REST 路由、
/// pod 内路径和 exec 命令参数属于“远端语义”，不可能是本地 Windows 路径，
/// 因此可以按同一张挂载表做逆变换还原原文；识别不了的形态原样返回。
///
/// 挂载表与 MSYS 运行时同源：隐式的安装根挂载（`/` → 安装根）加
/// `<安装根>/etc/fstab`（此处不再叠加 fstab.d 覆盖，Git 环境未使用）。

/// MSYS 安装根：POSIX 基础布局要求 usr/bin 必在安装根下，而调用方 bash 启动
/// 原生 exe 时必然把 usr/bin（Windows 形式）放进 PATH，据此反推安装根。
pub(crate) fn msys_install_root() -> Option<String> {
    let path = std::env::var_os("PATH")?;
    msys_root_from_entries(
        std::env::split_paths(&path).map(|entry| entry.to_string_lossy().into_owned()),
    )
}

pub(crate) fn msys_root_from_entries<I: IntoIterator<Item = String>>(
    entries: I,
) -> Option<String> {
    entries.into_iter().find_map(|entry| {
        let lower = entry.to_ascii_lowercase();
        ["\\usr\\bin", "/usr/bin"]
            .iter()
            .filter_map(|marker| lower.find(marker))
            .filter(|index| *index > 0)
            .min()
            .map(|index| entry[..index].trim_end_matches(['\\', '/']).to_string())
    })
}

/// 解析 fstab 文本（cygwin 约定：device mountpoint fstype flags...）。
/// `usertemp` 类型的挂载目标是“用户临时目录”，按 cygwin 语义以调用方传入的
/// 临时目录展开；`cygdrive` 是盘符前缀映射而非文件挂载，跳过。
/// 返回 (POSIX 挂载点, Windows 目标) 对，并追加隐式的安装根挂载。
pub(crate) fn msys_mount_table(fstab: &str, root: &str, usertemp: &str) -> Vec<(String, String)> {
    let mut table: Vec<(String, String)> = fstab
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut fields = line.split_whitespace();
            let device = fields.next()?;
            let mountpoint = fields.next()?;
            let fstype = fields.next()?;
            if !mountpoint.starts_with('/') {
                return None;
            }
            let target = match fstype {
                "usertemp" => usertemp,
                "cygdrive" => return None,
                _ => device,
            };
            let target = target.replace('\\', "/");
            let target = target.trim_end_matches('/');
            (!target.is_empty()).then(|| (mountpoint.to_owned(), target.to_owned()))
        })
        .collect();
    table.push((
        "/".to_owned(),
        root.trim_end_matches(['\\', '/']).replace('\\', "/"),
    ));
    table
}

/// 逆变换：`//x` 收敛为 `/x`（MSYS 约定的免转换写法）；Windows 形式的参数按
/// 挂载表最长前缀匹配还原为 POSIX 路径；前缀不落在任何挂载点边界上、或不在
/// 挂载表内的路径保持原样。
pub(crate) fn restore_msys_rewrite(raw: &str, table: &[(String, String)]) -> String {
    let mut path = raw;
    while path.starts_with("//") {
        path = &path[1..];
    }
    if path.starts_with('/') {
        return path.to_owned();
    }
    let candidate = path.replace('\\', "/");
    let mut best: Option<(usize, &str, &str)> = None;
    for (mountpoint, target) in table {
        if candidate.len() >= target.len()
            && let Some(prefix) = candidate.get(..target.len())
            && prefix.eq_ignore_ascii_case(target)
            && (candidate.len() == target.len() || candidate.as_bytes()[target.len()] == b'/')
            && best.as_ref().is_none_or(|(length, _, _)| target.len() > *length)
        {
            best = Some((target.len(), mountpoint, &candidate[target.len()..]));
        }
    }
    let Some((_, mountpoint, rest)) = best else {
        return path.to_owned();
    };
    let suffix = rest.strip_prefix('/').unwrap_or(rest);
    match (mountpoint, suffix.is_empty()) {
        (_, true) if mountpoint == "/" => "/".to_owned(),
        (_, true) => mountpoint.to_owned(),
        ("/", false) => format!("/{suffix}"),
        (_, false) => format!("{mountpoint}/{suffix}"),
    }
}

/// 还原“远端语义”参数。挂载表每个进程只解析一次；探测不到 MSYS 或解析不出
/// 挂载表时表为空，所有参数原样返回（交由调用方校验或服务端报错）。
pub(crate) fn un_mangle_remote_path(raw: &str) -> String {
    static TABLE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    let mut path = raw;
    while path.starts_with("//") {
        path = &path[1..];
    }
    if path.starts_with('/') {
        return path.to_owned();
    }
    let table = TABLE.get_or_init(|| {
        let Some(root) = msys_install_root() else {
            return Vec::new();
        };
        let fstab = std::fs::read_to_string(Path::new(&root).join("etc").join("fstab"))
            .unwrap_or_default();
        msys_mount_table(&fstab, &root, &local_temp_dir())
    });
    restore_msys_rewrite(path, table)
}

fn local_temp_dir() -> String {
    std::env::temp_dir()
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FSTAB: &str = "# 注释行\n\
                         none / cygdrive binary,posix=0,noacl,user 0 0\n\
                         none /tmp usertemp binary,posix=0,noacl 0 0\n\
                         C:/share /mnt/share ntfs binary 0 0\n";

    fn table() -> Vec<(String, String)> {
        msys_mount_table(FSTAB, "E:\\Git", "C:/Users/lenovo/AppData/Local/Temp")
    }

    #[test]
    fn install_root_is_derived_from_usr_bin_path_entries() {
        assert_eq!(
            msys_root_from_entries(["C:\\x".to_owned(), "E:\\Git\\usr\\bin".to_owned()]),
            Some("E:\\Git".to_owned())
        );
        // usr/bin 在开头（Linux 原生 PATH）或缺失时探测不到
        assert_eq!(msys_root_from_entries(["/usr/bin".to_owned()]), None);
        assert_eq!(msys_root_from_entries(["C:\\Windows\\system32".to_owned()]), None);
    }

    #[test]
    fn mount_table_follows_fstab_and_expands_usertemp() {
        let table = table();
        assert!(table.contains(&("/tmp".to_owned(), "C:/Users/lenovo/AppData/Local/Temp".to_owned())));
        assert!(table.contains(&("/mnt/share".to_owned(), "C:/share".to_owned())));
        assert!(table.contains(&("/".to_owned(), "E:/Git".to_owned())));
        assert!(!table.iter().any(|(mountpoint, _)| mountpoint == "/cygdrive"));
    }

    #[test]
    fn rewrites_are_restored_by_longest_prefix() {
        let table = table();
        assert_eq!(restore_msys_rewrite("E:/Git/job/detail/1", &table), "/job/detail/1");
        assert_eq!(restore_msys_rewrite("e:/git/job/detail/1", &table), "/job/detail/1");
        assert_eq!(
            restore_msys_rewrite("C:/Users/lenovo/AppData/Local/Temp/x.py", &table),
            "/tmp/x.py"
        );
        assert_eq!(restore_msys_rewrite("C:/Users/lenovo/AppData/Local/Temp", &table), "/tmp");
        assert_eq!(restore_msys_rewrite("C:/share/file", &table), "/mnt/share/file");
        assert_eq!(restore_msys_rewrite("//job/list", &table), "/job/list");
        assert_eq!(restore_msys_rewrite("/job/list", &table), "/job/list");
        // 前缀相同但不落在挂载点边界、或不在挂载表内的路径保持原样
        assert_eq!(restore_msys_rewrite("E:/Github/x", &table), "E:/Github/x");
        assert_eq!(restore_msys_rewrite("C:/Windows/system32", &table), "C:/Windows/system32");
    }

    #[test]
    fn empty_table_leaves_everything_unchanged() {
        assert_eq!(restore_msys_rewrite("E:/Git/job/list", &[]), "E:/Git/job/list");
    }
}
