#!/usr/bin/env bash
# 浏览器端到端测试的入口：装好 web-ui 依赖后运行 web-ui/e2e/ui.mjs。
# 用法: scripts/test-web-ui.sh /path/to/qd [port]
# 没有 Playwright 自带浏览器时，设置 PW_CHANNEL=chrome 用系统 Chrome，
# 或 PW_CHROMIUM=/path/to/chrome-headless-shell 指定可执行文件。
set -euo pipefail
BIN="$(cd "$(dirname "${1:?用法: test-web-ui.sh /path/to/qd [port]}")" && pwd)/$(basename "$1")"
cd "$(dirname "$0")/../web-ui"
[ -d node_modules/playwright ] || npm ci --no-audit --no-fund
node e2e/ui.mjs "$BIN" "${2:-28172}"
