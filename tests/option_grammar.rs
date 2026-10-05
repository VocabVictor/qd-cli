use std::fs;
use std::path::{Path, PathBuf};

/// Args::new 的归一化只认 src/cli/args.rs 里声明过的选项；本测试扫描源码中
/// take_option / option_u64 / take_option_any / take_flag / take_flag_any 的
/// 字面量实参，与 VALUE_OPTIONS / FLAG_OPTIONS 两张表比对，防止新增选项
/// 漏登记后在被前置书写时静默失效。

fn collect_sources(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("read source directory");
    for entry in entries {
        let path = entry.expect("read entry").path();
        if path.is_dir() {
            collect_sources(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn declared_options(args_rs: &str, table: &str) -> Vec<String> {
    let marker = format!("const {table}: &[&str] = &[");
    let start = args_rs
        .find(&marker)
        .unwrap_or_else(|| panic!("{table} 表不存在"))
        + marker.len();
    let end = start + args_rs[start..].find("];").expect("{table} 表未闭合");
    args_rs[start..end].split('"').skip(1).step_by(2).map(str::to_owned).collect()
}

/// 收集 `name(...)` 平衡括号内的 "-..." 字符串字面量；不含字面量的调用
/// （变量实参）不产生约束。
fn referenced_options(source: &str, call_names: &[&str]) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    for call in call_names {
        let mut search = 0;
        while let Some(relative) = source[search..].find(call) {
            let call_start = search + relative;
            search = call_start + call.len();
            if search >= bytes.len() || bytes[search] != b'(' {
                continue;
            }
            let mut depth = 0usize;
            let mut in_string = false;
            let mut literal = Vec::new();
            for &byte in &bytes[search..] {
                match byte {
                    b'"' => {
                        if in_string {
                            found.push(String::from_utf8_lossy(&literal).into_owned());
                            literal.clear();
                        }
                        in_string = !in_string;
                    }
                    _ if in_string => literal.push(byte),
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    found.retain(|value| value.starts_with('-') && value.len() > 1);
    found
}

#[test]
fn every_referenced_option_is_declared() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let args_rs =
        fs::read_to_string(root.join("src/cli/args.rs")).expect("read src/cli/args.rs");
    let value_options = declared_options(&args_rs, "VALUE_OPTIONS");
    let flag_options = declared_options(&args_rs, "FLAG_OPTIONS");

    let mut sources = Vec::new();
    collect_sources(&root.join("src"), &mut sources);
    let mut violations = Vec::new();
    for path in sources {
        let text = fs::read_to_string(&path).expect("read source");
        let name = path.strip_prefix(&root).unwrap_or(&path).display();
        for option in
            referenced_options(&text, &["take_option(", "option_u64(", "take_option_any(&["])
        {
            if !value_options.contains(&option) {
                violations.push(format!("{name}: {option} 未登记在 VALUE_OPTIONS"));
            }
        }
        for option in referenced_options(&text, &["take_flag(", "take_flag_any(&["]) {
            if !flag_options.contains(&option) {
                violations.push(format!("{name}: {option} 未登记在 FLAG_OPTIONS"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "选项语法表与源码不一致:\n{}",
        violations.join("\n")
    );
}
