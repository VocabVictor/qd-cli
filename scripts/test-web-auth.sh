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
expect "成员进开发机终端（权限放行，平台未登录）" 401
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

echo "=== 5c. 登录限制：单点登录、环境绑定、白名单 ==="
req POST /api/auth/register "$(jar carol)" "{\"username\":\"carol\",\"password\":\"carolpass1\",\"inviteCode\":\"$USER_CODE\"}" -H "X-QD-Device: devaaaa1111" -A "UA-one"
expect "注册 carol" 200
req GET /api/admin/login-policy "$(jar admin)"
has "默认策略是踢掉旧会话" '"sessionMode":"kick"'

# 默认 kick：第二处登录把第一处挤掉，并告诉第一处原因
req POST /api/auth/login "$(jar carol2)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devbbbb2222" -A "UA-two"
expect "carol 在第二处登录" 200
req GET /api/auth/state "$(jar carol)" "" -H "X-QD-Device: devaaaa1111" -A "UA-one"
has "第一处被挤下线" '"loggedIn":false'
has "第一处看到被挤原因" "别处登录"

# 绑定浏览器 + 设备：换 UA 或换设备都掉线
req GET /api/state "$(jar carol2)" "" -H "X-QD-Device: devbbbb2222" -A "UA-other"
expect "换浏览器后会话失效" 401
has "提示登录环境变了" "浏览器"
req POST /api/auth/login "$(jar carol3)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devcccc3333" -A "UA-three"
req GET /api/auth/state "$(jar carol3)" "" -H "X-QD-Device: devdddd4444" -A "UA-three"
has "换设备指纹后会话失效" '"loggedIn":false'

# deny 模式：在线时拒绝第二处登录
req POST /api/admin/login-policy "$(jar admin)" '{"sessionMode":"deny","bind":[],"ipAllow":[],"deviceAllow":[]}'
expect "管理员改成拒绝新登录" 200
req POST /api/auth/login "$(jar carol)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devaaaa1111"
expect "deny 模式首次登录" 200
req POST /api/auth/login "$(jar carol2)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devbbbb2222"
expect "deny 模式下第二处登录被拒" 403
has "拒绝原因说明正在别处使用" "只能在一处登录"
req POST /api/auth/logout "$(jar carol)" "{}"
req POST /api/auth/login "$(jar carol2)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devbbbb2222"
expect "第一处退出后第二处可以登录" 200

# 管理员不受限
req POST /api/auth/login "$(jar admin2)" '{"username":"boss","password":"bosspass123"}'
expect "deny 模式下管理员仍可多处登录" 200

# 在线会话与踢下线
req GET /api/admin/sessions "$(jar admin)"
expect "管理员列出在线会话" 200
has "会话列表含 carol 的设备指纹" '"device":"devbbbb2222"'
has "会话列表标出当前会话" '"current":true'
MINE=$(python3 -c 'import json,sys;d=json.load(open(sys.argv[1]));print(next(s["id"] for s in d["data"]["sessions"] if s.get("current")))' "$BODY" 2>/dev/null)
SID=$(python3 -c 'import json,sys;d=json.load(open(sys.argv[1]));print(next(s["id"] for s in d["data"]["sessions"] if s["username"]=="carol"))' "$BODY" 2>/dev/null)
req DELETE "/api/admin/sessions/$MINE" "$(jar admin)"
expect "不能踢掉自己的当前会话" 400
req DELETE "/api/admin/sessions/$SID" "$(jar admin)"
expect "管理员踢 carol 下线" 200
req GET /api/auth/state "$(jar carol2)" "" -H "X-QD-Device: devbbbb2222"
has "被踢后看到管理员下线提示" "管理员强制下线"

# IP 白名单（测试都从 127.0.0.1 发起）
req POST /api/admin/login-policy "$(jar admin)" '{"sessionMode":"off","bind":[],"ipAllow":["10.0.0.0/8"],"deviceAllow":[]}'
expect "设置不含本机的 IP 白名单" 200
req POST /api/auth/login "" '{"username":"carol","password":"carolpass1"}'
expect "白名单外 IP 登录被拒" 403
has "提示 IP 不在白名单" "白名单"
req POST /api/auth/register "" "{\"username\":\"dave\",\"password\":\"davepass12\",\"inviteCode\":\"$USER_CODE\"}"
expect "白名单外 IP 用普通邀请码注册被拒" 403
req POST /api/admin/login-policy "$(jar admin)" '{"sessionMode":"off","bind":[],"ipAllow":["127.0.0.0/8"],"deviceAllow":[]}'
req POST /api/auth/login "$(jar carol)" '{"username":"carol","password":"carolpass1"}'
expect "白名单内 IP 登录成功" 200
req POST /api/admin/login-policy "$(jar admin)" '{"sessionMode":"off","bind":[],"ipAllow":["10.0.0.0/8"],"deviceAllow":[]}'
req GET /api/state "$(jar carol)"
expect "收紧白名单后已登录成员立即失效" 401
req POST /api/admin/login-policy "$(jar admin)" '{"sessionMode":"off","bind":[],"ipAllow":["10.0.0.0/40"],"deviceAllow":[]}'
expect "非法 CIDR 被拒" 400

# 单个账号逐项覆盖：全局放开，carol 只单独设置设备白名单，其他项跟随全局
req POST /api/admin/login-policy "$(jar admin)" '{"sessionMode":"off","bind":[],"ipAllow":[],"deviceAllow":[]}'
req POST /api/admin/users/carol/login "$(jar admin)" '{"login":{"deviceAllow":{"mode":"replace","items":["devaaaa1111"]}}}'
expect "给 carol 单独设置设备白名单" 200
has "其他项仍跟随全局（单点登录 off）" '"sessionMode":"off"'
req POST /api/auth/login "" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devzzzz9999"
expect "白名单外设备登录被拒" 403
req POST /api/auth/login "$(jar carol)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devaaaa1111"
expect "白名单内设备登录成功" 200

# 全局收紧 IP：carol 只跟随这一项，所以立刻受影响
req POST /api/admin/login-policy "$(jar admin)" '{"sessionMode":"off","bind":[],"ipAllow":["10.0.0.0/8"],"deviceAllow":[]}'
req POST /api/auth/login "" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devaaaa1111"
expect "carol 的 IP 白名单跟随全局，全局收紧后被拒" 403
# 在全局之上为 carol 追加本机 IP
req POST /api/admin/users/carol/login "$(jar admin)" '{"login":{"ipAllow":{"mode":"extend","items":["127.0.0.1"]},"deviceAllow":{"mode":"replace","items":["devaaaa1111"]}}}'
expect "给 carol 在全局 IP 白名单上追加本机" 200
has "合并后的 IP 白名单含全局和追加项" '"ipAllow":["10.0.0.0/8","127.0.0.1"]'
req POST /api/auth/login "$(jar carol)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devaaaa1111"
expect "追加后 carol 能从本机登录" 200
req POST /api/auth/register "" "{\"username\":\"erin\",\"password\":\"erinpass12\",\"inviteCode\":\"$USER_CODE\"}"
expect "其他成员仍受全局 IP 白名单约束（注册被拒）" 403

# 单独设置单点登录：先用 kick 清掉前面不限制模式下留下的多个会话，再测 deny
req POST /api/admin/users/carol/login "$(jar admin)" '{"login":{"sessionMode":"kick","ipAllow":{"mode":"replace","items":[]}}}'
expect "carol 单独设为踢掉旧会话" 200
req POST /api/auth/login "$(jar carol)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devaaaa1111"
req POST /api/auth/logout "$(jar carol)" "{}"
req POST /api/admin/users/carol/login "$(jar admin)" '{"login":{"sessionMode":"deny","ipAllow":{"mode":"replace","items":[]}}}'
expect "carol 单独设为拒绝新登录、IP 不限" 200
req POST /api/auth/login "$(jar carol)" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devaaaa1111"
expect "carol 第一处登录" 200
req POST /api/auth/login "" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devbbbb2222"
expect "carol 单独的 deny 生效：第二处被拒" 403

# 免除限制
req POST /api/admin/users/carol/login "$(jar admin)" '{"login":{"exempt":true}}'
expect "免除 carol 的登录限制" 200
has "免除后实际生效为不受限" '"effective":null'
req POST /api/auth/login "" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devbbbb2222"
expect "免除后可同时多处登录" 200

req POST /api/admin/users/carol/login "$(jar admin)" '{"login":{"ipAllow":{"mode":"extend","items":["bad-ip"]}}}'
expect "覆盖里的非法 IP 被拒" 400
req POST /api/admin/users/carol/login "$(jar admin)" '{"login":null}'
expect "carol 全部恢复跟随全局" 200
req POST /api/auth/login "" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devzzzz9999"
expect "恢复全局后受全局 IP 白名单约束" 403
req POST /api/admin/login-policy "$(jar admin)" '{"sessionMode":"off","bind":[],"ipAllow":[],"deviceAllow":[]}'
req POST /api/auth/login "" '{"username":"carol","password":"carolpass1"}' -H "X-QD-Device: devzzzz9999"
expect "全局放开后任意设备可登录" 200

echo "=== 6. 文件权限 ==="
for f in web-accounts.json web-login.json; do
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
