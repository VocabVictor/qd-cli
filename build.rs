// 构建时把前端单文件包放到 OUT_DIR，供 src/web/mod.rs 通过 include_str! 内嵌。
// index.html 是构建产物，不进 git。
//
// 三种情况：
// 1. 默认：有 npm 就用 npm 重新构建前端。build.rs 只在声明的路径变化时重跑，
//    所以一旦重跑就必须重新构建——曾经这里是 `if !dist.exists()`，结果改完
//    web-ui/src 后仍把陈旧 dist 拷进二进制，前端改动被静默丢弃。
// 2. QD_SKIP_WEB_BUILD=1：跳过 npm，直接用已有的 web-ui/dist（CI 已单独构建过前端时用）。
// 3. 没有 Node：退回已有的 dist；连 dist 都没有就内嵌占位页。两者都会打 cargo 警告，
//    这样只改 Rust 的人不必装 Node。
use std::{env, fs, path::Path, process::Command};

const PLACEHOLDER: &str = "<!doctype html><meta charset=\"utf-8\"><title>qd web</title>\
<p>此 qd 构建未包含网页控制台：编译时没有 Node.js，也没有现成的 web-ui/dist。</p>";

fn main() {
    println!("cargo:rerun-if-changed=web-ui/src");
    println!("cargo:rerun-if-changed=web-ui/index.html");
    println!("cargo:rerun-if-changed=web-ui/package.json");
    println!("cargo:rerun-if-changed=web-ui/package-lock.json");
    println!("cargo:rerun-if-env-changed=QD_SKIP_WEB_BUILD");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let dest = Path::new(&out_dir).join("index.html");
    let dist = Path::new("web-ui").join("dist").join("index.html");
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };

    let skip = env::var("QD_SKIP_WEB_BUILD").is_ok_and(|value| !value.is_empty() && value != "0");
    if !skip && npm_available(npm) {
        build_frontend(npm);
        assert!(dist.exists(), "前端构建未产出 web-ui/dist/index.html");
    }

    if dist.exists() {
        if skip {
            println!("cargo:rerun-if-changed=web-ui/dist/index.html");
        } else if !npm_available(npm) {
            println!("cargo:warning=未找到 npm，内嵌的是现有的 web-ui/dist，可能不是最新前端");
            println!("cargo:rerun-if-changed=web-ui/dist/index.html");
        }
        fs::copy(&dist, &dest).expect("复制前端 index.html 到 OUT_DIR 失败");
    } else {
        println!("cargo:warning=没有 npm 也没有 web-ui/dist，qd web 将只显示占位页");
        fs::write(&dest, PLACEHOLDER).expect("写入占位页失败");
    }
}

fn npm_available(npm: &str) -> bool {
    Command::new(npm)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn build_frontend(npm: &str) {
    if !Path::new("web-ui").join("node_modules").exists() {
        // 只用 npm ci：失败就报错，不悄悄退回 npm install 去改写 package-lock.json。
        let installed = Command::new(npm)
            .args(["ci", "--no-audit", "--no-fund"])
            .current_dir("web-ui")
            .status()
            .expect("运行 npm ci 失败");
        assert!(installed.success(), "npm ci 失败（web-ui）");
    }
    let built = Command::new(npm)
        .args(["run", "build"])
        .current_dir("web-ui")
        .status()
        .expect("运行前端构建失败");
    assert!(built.success(), "前端构建失败（web-ui npm run build）");
}
