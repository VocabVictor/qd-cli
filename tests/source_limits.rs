use std::{fs, path::PathBuf};

const MAX_RUST_LINES: usize = 300;

#[test]
fn every_rust_source_stays_within_line_limit() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut pending = vec![root.join("src"), root.join("tests")];
    let mut violations = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read source directory") {
            let path = entry.expect("read source entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                let source = fs::read_to_string(&path).expect("read Rust source");
                let lines = source.lines().count();
                if lines > MAX_RUST_LINES {
                    violations.push(format!(
                        "{}: {lines} lines",
                        path.strip_prefix(&root).unwrap_or(&path).display()
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Rust files must not exceed {MAX_RUST_LINES} lines:\n{}",
        violations.join("\n")
    );
}
