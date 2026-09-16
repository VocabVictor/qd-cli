// 构建时生成前端包到 OUT_DIR，供 src/web/mod.rs 通过 include_str! 内嵌。
// index.html 是构建产物，不进 git；每次 build.rs 重跑都用 npm 重新构建（需 Node）。
use std::{env, fs, path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=web-ui/src");
    println!("cargo:rerun-if-changed=web-ui/index.html");
    println!("cargo:rerun-if-changed=web-ui/package.json");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("index.html");
    let dist = Path::new("web-ui").join("dist").join("index.html");

    // build.rs 只在上面声明的路径变化时重跑，所以一旦重跑就必须重新构建前端。
    // 曾经这里是 `if !dist.exists()`，结果本地改完 web-ui/src 后仍然把上一次的
    // 陈旧 dist 拷进二进制，前端改动被静默丢弃。
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    if !Path::new("web-ui").join("node_modules").exists() {
        let install = Command::new(npm)
            .args(["ci", "--no-audit", "--no-fund"])
            .current_dir("web-ui")
            .status();
        if !matches!(install, Ok(s) if s.success()) {
            let _ = Command::new(npm).args(["install", "--no-audit", "--no-fund"]).current_dir("web-ui").status();
        }
    }
    let built = Command::new(npm)
        .args(["run", "build"])
        .current_dir("web-ui")
        .status()
        .expect("运行前端构建失败：请确认已安装 Node.js");
    assert!(built.success(), "前端构建失败（web-ui npm run build）");
    assert!(dist.exists(), "前端构建未产出 web-ui/dist/index.html");

    fs::copy(&dist, &dest).expect("复制前端 index.html 到 OUT_DIR 失败");
}
