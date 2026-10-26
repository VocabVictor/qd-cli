#!/usr/bin/env bash
# qd web 本机账号墙的黑盒回归测试：起一个真实的 qd web 进程，用 curl 打它的
# HTTP 接口，断言状态码和响应体。覆盖公开绑定（注册/登录/登出/session 门槛）
# 和本机回环绑定（确认完全不受新账号墙影响，老用法零改动）两条路径。
#
# 用法: scripts/test-web-auth.sh /path/to/qd
set -uo pipefail

BIN="${1:?用法: test-web-auth.sh /path/to/qd}"
PORT=18080
INVITE="ci-invite-$$"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/qd"
ACCOUNTS_FILE="$CONFIG_DIR/web-accounts.json"
COOKIE_JAR="$(mktemp)"
BODY_FILE="$(mktemp)"
LOG_FILE="$(mktemp)"
PASS=0
FAIL=0
SERVER_PID=""

cleanup() {
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" >/dev/null 2>&1
  rm -f "$COOKIE_JAR" "$BODY_FILE" "$LOG_FILE"
}
trap cleanup EXIT

pass() { echo "PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "FAIL: $1"; FAIL=$((FAIL + 1)); }

# req METHOD PATH [COOKIE_JAR] [BODY_JSON] -> 把状态码写进 $CODE，响应体写进 $BODY_FILE
req() {
  local method="$1" path="$2" jar="${3:-}" data="${4:-}"
  local args=(-s -o "$BODY_FILE" -w '%{http_code}' -X "$method" "http://127.0.0.1:$PORT$path")
  [ -n "$jar" ] && args+=(-b "$jar" -c "$jar")
  [ -n "$data" ] && args+=(-H 'Content-Type: application/json' -d "$data")
  CODE=$(curl "${args[@]}")
}

assert_code() {
  local desc="$1" expected="$2"
  if [ "$CODE" = "$expected" ]; then pass "$desc (HTTP $CODE)"; else fail "$desc (期望 $expected，实得 $CODE，body: $(cat "$BODY_FILE"))"; fi
}

assert_body_has() {
  local desc="$1" needle="$2"
  if grep -qF "$needle" "$BODY_FILE"; then pass "$desc"; else fail "$desc (body: $(cat "$BODY_FILE"))"; fi
}

wait_for_server() {
  for _ in $(seq 1 30); do
    if curl -s -o /dev/null "http://127.0.0.1:$PORT/"; then return 0; fi
    sleep 0.2
  done
  echo "服务没能在超时内启动，日志:"; cat "$LOG_FILE"; exit 1
}

start_server() {
  rm -f "$ACCOUNTS_FILE"
  if [ "$1" = "public" ]; then
    "$BIN" web --host 0.0.0.0 --port "$PORT" --password "$INVITE" >"$LOG_FILE" 2>&1 &
  else
    "$BIN" web --host 127.0.0.1 --port "$PORT" >"$LOG_FILE" 2>&1 &
  fi
  SERVER_PID=$!
  wait_for_server
}

stop_server() {
  kill "$SERVER_PID" >/dev/null 2>&1
  wait "$SERVER_PID" 2>/dev/null
  SERVER_PID=""
}

echo "=== 场景 1: 公开绑定不带 --password/QD_WEB_PASSWORD 必须拒绝启动 ==="
if "$BIN" web --host 0.0.0.0 --port "$PORT" >"$LOG_FILE" 2>&1; then
  fail "无邀请码仍然启动成功了"
else
  pass "无邀请码拒绝启动"
fi
grep -q "邀请码" "$LOG_FILE" && pass "错误信息提到邀请码" || fail "错误信息没提到邀请码: $(cat "$LOG_FILE")"

echo "=== 场景 2: 公开绑定 + 账号墙全流程 ==="
start_server public

req GET /api/auth/state
assert_code "初始 /api/auth/state 可匿名访问" 200
assert_body_has "初始状态 required=true" '"required":true'
assert_body_has "初始状态 hasAccount=false" '"hasAccount":false'
assert_body_has "初始状态 loggedIn=false" '"loggedIn":false'

req GET /api/state
assert_code "未登录时平台 /api/state 直接 401（账号墙先挡住）" 401

req POST /api/auth/register "" "{\"username\":\"alice\",\"password\":\"hunter22\",\"inviteCode\":\"wrong-code\"}"
assert_code "邀请码错误时注册被拒" 401

req POST /api/auth/register "" '{"username":"alice","password":"short","inviteCode":"'"$INVITE"'"}'
assert_code "密码太短时注册被拒" 400

req POST /api/auth/register "$COOKIE_JAR" "{\"username\":\"alice\",\"password\":\"hunter22\",\"inviteCode\":\"$INVITE\"}"
assert_code "邀请码正确时注册成功" 200
grep -qi "qd_session" "$COOKIE_JAR" && pass "注册成功签发了 qd_session cookie" || fail "没拿到 qd_session cookie"

req POST /api/auth/register "" "{\"username\":\"alice\",\"password\":\"hunter22\",\"inviteCode\":\"$INVITE\"}"
assert_code "重复用户名注册被拒" 400

req GET /api/state "$COOKIE_JAR"
assert_code "带 session cookie 能访问平台 /api/state" 200

req GET /api/state
assert_code "不带 cookie 访问平台 /api/state 仍是 401" 401

BAD_JAR="$(mktemp)"
echo -e "127.0.0.1\tFALSE\t/\tFALSE\t0\tqd_session\tnot-a-real-token" > "$BAD_JAR"
req GET /api/state "$BAD_JAR"
assert_code "伪造的 session token 被拒" 401
rm -f "$BAD_JAR"

req POST /api/auth/login "" '{"username":"alice","password":"wrong-password"}'
assert_code "密码错误登录被拒" 401

req POST /api/auth/login "" '{"username":"nobody","password":"hunter22"}'
assert_code "用户不存在登录被拒" 401

LOGIN_JAR="$(mktemp)"
req POST /api/auth/login "$LOGIN_JAR" '{"username":"alice","password":"hunter22"}'
assert_code "密码正确登录成功" 200

req GET /api/auth/state "$LOGIN_JAR"
assert_body_has "登录后 hasAccount=true" '"hasAccount":true'
assert_body_has "登录后 loggedIn=true" '"loggedIn":true'

req POST /api/auth/logout "$LOGIN_JAR"
assert_code "登出成功" 200
req GET /api/state "$LOGIN_JAR"
assert_code "登出后同一 cookie 不再能访问平台 /api/state" 401
rm -f "$LOGIN_JAR"

req POST /api/auth/register "" "{\"username\":\"bob\",\"password\":\"anotherpass1\",\"inviteCode\":\"$INVITE\"}"
assert_code "第二个账号也能用同一邀请码注册" 200

if [ -e "$ACCOUNTS_FILE" ]; then
  PERM=$(stat -c '%a' "$ACCOUNTS_FILE" 2>/dev/null || stat -f '%Lp' "$ACCOUNTS_FILE" 2>/dev/null)
  [ "$PERM" = "600" ] && pass "web-accounts.json 权限收紧到 600" || fail "web-accounts.json 权限是 $PERM，不是 600"
else
  fail "web-accounts.json 没有被创建"
fi

grep -q "裸 HTTP" "$LOG_FILE" && pass "公开绑定打印了 TLS 风险警告" || fail "没看到 TLS 风险警告: $(cat "$LOG_FILE")"

stop_server

echo "=== 场景 3: 本机回环绑定——账号墙必须完全不介入（老用法零改动） ==="
start_server local

req GET /api/auth/state
assert_body_has "本机绑定 required=false" '"required":false'
assert_body_has "本机绑定 loggedIn=true（视为已通过）" '"loggedIn":true'

req GET /api/state
assert_code "本机绑定下平台 /api/state 不需要 cookie 就能访问" 200

req POST /api/config "" '{"key":"concurrency","value":"16"}'
assert_code "本机绑定下无 Origin 头的写请求被 origin_guard 拒绝（跟账号墙无关的既有行为）" 403

stop_server

echo
echo "通过 $PASS / 失败 $FAIL"
[ "$FAIL" -eq 0 ]
