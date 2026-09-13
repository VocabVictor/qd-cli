// 构建时生成前端包到 OUT_DIR，供 src/web/mod.rs 通过 include_str! 内嵌。
// index.html 是构建产物，不进 git；若 web-ui/dist 缺失则用 npm 现场构建（需 Node）。
use std::{env, fs, path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=web-ui/dist/index.html");
    println!("cargo:rerun-if-changed=web-ui/src");
    println!("cargo:rerun-if-changed=web-ui/index.html");
    println!("cargo:rerun-if-changed=web-ui/package.json");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("index.html");
    let dist = Path::new("web-ui").join("dist").join("index.html");

    if !dist.exists() {
        let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
        let install = Command::new(npm)
            .args(["ci", "--no-audit", "--no-fund"])
            .current_dir("web-ui")
            .status();
        if !matches!(install, Ok(s) if s.success()) {
            let _ = Command::new(npm).args(["install", "--no-audit", "--no-fund"]).current_dir("web-ui").status();
        }
        let built = Command::new(npm)
            .args(["run", "build"])
            .current_dir("web-ui")
            .status()
            .expect("运行前端构建失败：请确认已安装 Node.js");
        assert!(built.success(), "前端构建失败（web-ui npm run build）");
    }
    fs::copy(&dist, &dest).expect("复制前端 index.html 到 OUT_DIR 失败");
}
