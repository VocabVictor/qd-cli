#!/usr/bin/env bash
# qd web 本机账号/角色/配额的黑盒回归：起真实 qd web 进程，用 curl 打 HTTP 接口断言。
# 在临时目录里跑（XDG_CONFIG_HOME/XDG_STATE_HOME 指向临时目录），不碰真实账号和平台登录态，
# 也不会向平台提交任何作业——所有被测的拒绝路径都在请求平台之前返回。
#
# 用法: scripts/test-web-auth.sh /path/to/qd [port]
set -uo pipefail

BIN="$(cd "$(dirname "${1:?用法: test-web-auth.sh /path/to/qd [port]}")" && pwd)/$(basename "$1")"
PORT="${2:-28171}"
BASE="http://127.0.0.1:$PORT"
WORK="$(mktemp -d)"
export XDG_CONFIG_HOME="$WORK/config" XDG_STATE_HOME="$WORK/state" HOME="$WORK/home"
unset APPDATA LOCALAPPDATA QD_TOKEN
mkdir -p "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$HOME"
USER_CODE="user-invite-$RANDOM-$RANDOM"
ADMIN_CODE="admin-invite-$RANDOM-$RANDOM"
BODY="$WORK/body"
LOG="$WORK/server.log"
PASS=0
FAIL=0
SERVER_PID=""

cleanup() {
  [ -n "$SERVER_PID" ] && kill "$SERVER_PID" >/dev/null 2>&1
  rm -rf "$WORK"
}
trap cleanup EXIT

pass() { echo "PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "FAIL: $1"; FAIL=$((FAIL + 1)); }

# req METHOD PATH JAR [JSON] [extra curl args...] -> $CODE，响应体在 $BODY
req() {
  local method="$1" path="$2" jar="$3" data="${4:-}"
  shift 4 2>/dev/null || shift $#
  local args=(-s -o "$BODY" -w '%{http_code}' -X "$method" "$BASE$path")
  [ -n "$jar" ] && args+=(-b "$jar" -c "$jar")
  [ -n "$data" ] && args+=(-H 'Content-Type: application/json' -d "$data")
  CODE=$(curl "${args[@]}" "$@")
}
expect() {
  local desc="$1" want="$2"
  if [ "$CODE" = "$want" ]; then pass "$desc (HTTP $CODE)"; else fail "$desc (期望 $want，实得 $CODE: $(head -c 300 "$BODY"))"; fi
}
has() {
  if grep -qF "$2" "$BODY"; then pass "$1"; else fail "$1 (body: $(head -c 300 "$BODY"))"; fi
}
jar() { echo "$WORK/$1.cookies"; }

# 直接后台运行二进制（不经 shell 函数），保证 $! 就是 qd 进程本身，kill 时不会漏掉。
start_server() {
  rm -f "$XDG_CONFIG_HOME/qd/web-accounts.json" "$XDG_CONFIG_HOME/qd/web-jobs.json"
  if curl -s -o /dev/null "$BASE/"; then
    echo "端口 $PORT 已被占用，拒绝在别的进程上跑测试"; exit 1
  fi
  "$BIN" --base-url https://platform.invalid web "$@" >"$LOG" 2>&1 &
  SERVER_PID=$!
  for _ in $(seq 1 50); do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then echo "服务启动即退出:"; cat "$LOG"; exit 1; fi
    curl -s -o /dev/null "$BASE/" && return 0
    sleep 0.2
  done
  echo "服务没能启动:"; cat "$LOG"; exit 1
}
stop_server() {
  kill "$SERVER_PID" >/dev/null 2>&1
  wait "$SERVER_PID" 2>/dev/null
  SERVER_PID=""
  for _ in $(seq 1 25); do curl -s -o /dev/null "$BASE/" || return 0; sleep 0.2; done
  echo "旧服务没有退出"; exit 1
}

startup_must_fail() {
  local desc="$1" needle="$2"; shift 2
  if env "$@" "$BIN" --base-url https://platform.invalid web --host 0.0.0.0 --port "$PORT" >"$LOG" 2>&1; then
    fail "$desc：竟然启动成功"
  elif grep -qF "$needle" "$LOG"; then
    pass "$desc"
  else
    fail "$desc：错误信息不含「$needle」: $(cat "$LOG")"
  fi
}

echo "=== 1. 启动参数校验 ==="
startup_must_fail "缺少两个邀请码拒绝启动" "QD_WEB_ADMIN_INVITE_CODE"
startup_must_fail "只给普通邀请码拒绝启动" "QD_WEB_ADMIN_INVITE_CODE" QD_WEB_INVITE_CODE="$USER_CODE"
startup_must_fail "两个邀请码相同拒绝启动" "不能相同" QD_WEB_INVITE_CODE="$USER_CODE" QD_WEB_ADMIN_INVITE_CODE="$USER_CODE"
startup_must_fail "邀请码过短拒绝启动" "至少 12 位" QD_WEB_INVITE_CODE=short QD_WEB_ADMIN_INVITE_CODE=short-admin
if "$BIN" --base-url https://platform.invalid web --host 0.0.0.0 --port "$PORT" --password x >"$LOG" 2>&1; then
  fail "旧的 --password 参数仍被接受"
else
  grep -qF -- "--invite-code" "$LOG" && pass "旧的 --password 给出迁移提示" || fail "--password 报错信息不对: $(cat "$LOG")"
fi

echo "=== 2. 公开绑定：注册与角色 ==="
export QD_WEB_INVITE_CODE="$USER_CODE" QD_WEB_ADMIN_INVITE_CODE="$ADMIN_CODE"
start_server --host 0.0.0.0 --port "$PORT"
grep -q "裸 HTTP" "$LOG" && pass "启动时打印 TLS 风险警告" || fail "没有 TLS 警告"
ps -o args= -p "$SERVER_PID" 2>/dev/null | grep -qF "$ADMIN_CODE" && fail "邀请码出现在进程命令行里" || pass "邀请码不在进程命令行里（环境变量传入）"

req GET /api/auth/state ""
expect "未登录可读 /api/auth/state" 200
has "初始 hasAccount=false" '"hasAccount":false'

req POST /api/auth/register "" '{"username":"mallory","password":"password123","inviteCode":"wrong-code-000"}'
expect "错误邀请码注册被拒" 401

req POST /api/auth/register "$(jar admin)" "{\"username\":\"boss\",\"password\":\"bosspass123\",\"inviteCode\":\"$ADMIN_CODE\"}"
expect "管理员邀请码注册成功" 200
has "管理员码注册得到 role=admin" '"role":"admin"'

req POST /api/auth/register "$(jar alice)" "{\"username\":\"alice\",\"password\":\"alicepass1\",\"inviteCode\":\"$USER_CODE\"}"
expect "普通邀请码注册成功" 200
has "普通码注册得到 role=user" '"role":"user"'

req POST /api/auth/register "" "{\"username\":\"ALICE\",\"password\":\"alicepass1\",\"inviteCode\":\"$USER_CODE\"}"
expect "用户名大小写不同也视为重复" 400
req POST /api/auth/register "" "{\"username\":\"bad name!\",\"password\":\"alicepass1\",\"inviteCode\":\"$USER_CODE\"}"
expect "非法用户名被拒" 400
req POST /api/auth/register "" "{\"username\":\"shorty\",\"password\":\"short\",\"inviteCode\":\"$USER_CODE\"}"
expect "过短密码被拒" 400

req GET /api/auth/state "$(jar alice)"
has "成员默认总配额为 0" '"total":0'
has "成员状态里 role=user" '"role":"user"'

echo "=== 3. 未登录 / 跨站 ==="
for path in /api/state /api/jobs /static/terminal.js /terminal /ws/ns/pod/c/t/webshell /api/admin/users; do
  req GET "$path" ""
  expect "未登录访问 $path" 401
done
req POST /api/auth/logout "$(jar alice)" "{}" -H "Origin: https://evil.example"
expect "跨站 Origin 写请求被拒" 403

echo "=== 4. 成员权限边界（都在请求平台之前拒绝） ==="
for spec in "POST /api/config {\"key\":\"concurrency\",\"value\":\"1\"}" \
            "POST /api/logout {}" \
            "POST /api/login {\"username\":\"x\",\"password\":\"y\"}" \
            "POST /api/keepalive {\"action\":\"status\"}" \
            "POST /api/templates {\"name\":\"t\",\"kind\":\"job\",\"payload\":{}}" \
            "GET /api/admin/users " \
            "POST /api/admin/users/alice/quota {\"total\":8}"; do
  method="${spec%% *}"; rest="${spec#* }"; path="${rest%% *}"; data="${rest#* }"
  req "$method" "$path" "$(jar alice)" "$data"
  expect "成员调用管理员接口 $method $path" 403
done
req POST /api/call "$(jar alice)" '{"method":"POST","service":"core","path":"/job/new","data":{}}'
expect "成员通过透传接口发写请求" 403
req POST /api/call "$(jar alice)" '{"method":"DELETE","service":"core","path":"/job/delete/1","data":{}}'
expect "成员通过透传接口发 DELETE" 403

GPU_JOB='{"jobName":"t","spaceId":"s1","rsgroupId":"g1","projectId":"p1","taskroles":[{"instance":2,"gpu":1}]}'
req POST /api/job/submit "$(jar alice)" "$GPU_JOB"
expect "成员配额 0 时提交 GPU 作业被拒" 403
has "配额拒绝原因提到总配额" "总配额"

req POST /api/bulk "$(jar alice)" '{"action":"job-cancel","ids":["999999"],"spaceId":"s1"}'
expect "成员取消不属于自己的作业" 403
req POST /api/bulk "$(jar alice)" '{"action":"job-delete","ids":["999999"],"spaceId":"s1"}'
expect "成员删除不属于自己的作业" 403
req GET "/api/terminal-url?kind=job&id=999999" "$(jar alice)"
expect "成员进不属于自己作业的终端" 403
req GET "/api/terminal-url?kind=dev&id=123" "$(jar alice)"
expect "成员进开发机终端" 403
req POST /api/exec "$(jar alice)" '{"kind":"job","id":"999999","command":["id"]}'
expect "成员在别人作业里 exec" 403

echo "=== 5. 管理员：配额、角色、密码、删除 ==="
req GET /api/admin/users "$(jar admin)"
expect "管理员列出用户" 200
has "用户列表含 alice" '"username":"alice"'

req POST /api/admin/users/alice/quota "$(jar admin)" '{"total":2,"spaces":{"s1":2},"groups":{"g1":1}}'
expect "管理员设置配额" 200
req POST /api/admin/users/alice/quota "$(jar admin)" '{"total":-1}'
expect "非法配额被拒" 400
req POST /api/admin/users/alice/quota "$(jar admin)" '{"total":1,"spaces":{"../x":1}}'
expect "非法空间 ID 被拒" 400

req GET /api/auth/state "$(jar alice)"
has "成员看到新的总配额" '"total":2'
req POST /api/job/submit "$(jar alice)" "$GPU_JOB"
expect "资源组配额 1 时提交 2 卡作业被拒" 403
has "拒绝原因指向资源组" "资源组 g1"

req POST /api/admin/users/boss/role "$(jar admin)" '{"role":"user"}'
expect "不能降级最后一个管理员" 400
req DELETE /api/admin/users/boss "$(jar admin)"
expect "不能删除最后一个管理员" 400

req POST /api/admin/users/alice/role "$(jar admin)" '{"role":"admin"}'
expect "提升 alice 为管理员" 200
req GET /api/admin/users "$(jar alice)"
expect "提升后 alice 立即能用管理接口" 200
req POST /api/admin/users/alice/role "$(jar admin)" '{"role":"user"}'
expect "降级 alice" 200
req GET /api/admin/users "$(jar alice)"
expect "降级后 alice 立即失去管理权限" 403

cp "$(jar alice)" "$(jar alice-old)"
req POST /api/auth/password "$(jar alice)" '{"oldPassword":"wrong-old-pass","newPassword":"newalicepass"}'
expect "旧密码错误时改密被拒" 401
req POST /api/auth/password "$(jar alice)" '{"oldPassword":"alicepass1","newPassword":"newalicepass"}'
expect "成员修改自己的密码" 200
req GET /api/auth/state "$(jar alice-old)"
has "改密后其他旧 session 失效" '"loggedIn":false'
req GET /api/state "$(jar alice)"
expect "改密后当前会话换发了新 cookie 仍可用" 200
req POST /api/auth/login "" '{"username":"alice","password":"newalicepass"}'
expect "用新密码登录" 200

req POST /api/admin/users/alice/password "$(jar admin)" '{"password":"resetbyadmin"}'
expect "管理员重置成员密码" 200
req GET /api/state "$(jar alice)"
expect "重置密码后成员原 session 失效" 401

req POST /api/auth/login "$(jar alice)" '{"username":"Alice","password":"resetbyadmin"}'
expect "用户名大小写不敏感登录" 200
req DELETE /api/admin/users/alice "$(jar admin)"
expect "管理员删除成员" 200
req GET /api/state "$(jar alice)"
expect "被删成员的 session 立即失效" 401

echo "=== 6. 文件权限 ==="
for f in web-accounts.json; do
  path="$XDG_CONFIG_HOME/qd/$f"
  perm=$(stat -c '%a' "$path" 2>/dev/null || stat -f '%Lp' "$path" 2>/dev/null)
  [ "$perm" = "600" ] && pass "$f 权限为 600" || fail "$f 权限是 $perm"
done

echo "=== 7. 登录限速（放最后：会锁住本机 IP） ==="
# 前面场景里错误邀请码、错误旧密码也计入同一 IP，所以只断言"10 次以内必然锁定"。
LOCKED_AT=""
for i in $(seq 1 11); do
  req POST /api/auth/login "" '{"username":"boss","password":"wrong-password"}'
  if [ "$CODE" = "429" ]; then LOCKED_AT=$i; break; fi
  [ "$CODE" = "401" ] || fail "第 $i 次错误密码返回了 $CODE"
done
[ -n "$LOCKED_AT" ] && pass "连续错误密码第 $LOCKED_AT 次起返回 429" || fail "11 次错误密码后仍未锁定"
has "429 提示稍后再试" "分钟后再试"
req POST /api/auth/login "" '{"username":"boss","password":"bosspass123"}'
expect "锁定期间正确密码也被拒" 429
stop_server

echo "=== 8. 本机回环绑定：老用法不变 ==="
unset QD_WEB_INVITE_CODE QD_WEB_ADMIN_INVITE_CODE
start_server --host 127.0.0.1 --port "$PORT"
req GET /api/auth/state ""
has "本机绑定 required=false" '"required":false'
req GET /api/state ""
expect "本机绑定无需 cookie 访问 /api/state" 200
req POST /api/config "" '{"key":"concurrency","value":"16"}'
expect "本机绑定无 Origin 的写请求仍被 origin_guard 拒绝" 403
req POST /api/config "" '{"key":"concurrency","value":"16"}' -H "Origin: http://127.0.0.1:$PORT"
expect "本机绑定同源页面可改配置（视为管理员）" 200
stop_server

echo
echo "通过 $PASS / 失败 $FAIL"
[ "$FAIL" -eq 0 ]
