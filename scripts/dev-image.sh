#!/usr/bin/env bash
# 开发机镜像构建脚本（CPU 版）
#
# 用法：在一台以 ubuntu:24.04 为基底的开发环境里跑，跑完用平台的
# 「另存为」存成普通镜像。镜像 tag 请带上本仓库的 commit 短 sha，
# 这样镜像里装了什么可以回溯到这个文件。
#
# 刻意不装：torch / CUDA / transformers 等 —— CPU 开发机用不上，
# 而且会把镜像从 1.7 GB 撑到 24 GB。GPU 开发机另走 NGC 镜像。

set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

log() { echo "[$(date +%H:%M:%S)] == $* =="; }

# ------------------------------------------------------- tzdata 的坑
# 平台把 /etc/localtime 作为 bind mount 挂进容器（让容器时区跟节点一致）。
# tzdata 的 postinst 要把新时区 "mv -f" 覆盖到 /etc/localtime，而 rename
# 覆盖一个挂载点必然失败：
#   mv: cannot move '/etc/localtime.dpkg-new' to '/etc/localtime':
#       Device or resource busy
# 一失败就连坐：libpython3.12-stdlib -> python3.12 -> python3 ->
# python3-pip/venv 全部配置不了。
#
# 注意 dpkg 的 path-exclude 在这里没用——动手的是包自己的 postinst 脚本，
# 不是 dpkg 解包。所以直接把那句 mv 改成删掉临时文件：宿主机的
# /etc/localtime 本来就是我们想要的时区，不需要动它。
#
# 只有用国内镜像源时才会踩到：官方源给的 tzdata 与基础镜像同版本，不触发
# 升级；清华源有 noble-updates 里更新的 tzdata，会触发。
patch_tzdata() {
  local f=/var/lib/dpkg/info/tzdata.postinst
  [ -f "$f" ] || return 0
  # 只替换 mv 那一段，不用反向引用： 经过多层转义容易被写成字面的
  # 0x01 字节，结果命令名变成 "rm"，报 "rm: not found"（exit 127）。
  # 换成 shell 内建 : ，既不依赖外部命令，缩进也原样保留。
  sed -i 's|mv -f "$DPKG_ROOT/etc/localtime.dpkg-new" "$DPKG_ROOT/etc/localtime"|:|' "$f"
  # 修掉上一版补丁可能留下的损坏行
  sed -i 's|^.rm -f "$DPKG_ROOT/etc/localtime.dpkg-new"$|:|' "$f"
}

# ---------------------------------------------------------------- apt 源
# 顺序很重要：裸 ubuntu:24.04 里没有 ca-certificates，没有根证书就校验
# 不了 TLS。所以必须先用默认的 http 源把 ca-certificates 装上，之后才
# 能把源换成 https 的清华镜像——反过来做会死锁，apt 连不上任何源，表现
# 为所有包都 "Unable to locate package"。
log "先用默认源装 ca-certificates（换 https 源的前提）"
apt-get update -qq
apt-get install -y --no-install-recommends ca-certificates || true
patch_tzdata
dpkg --configure -a

# ------------------------------------------------------------- chsrc
# 换源不再写死某一家。实测同一天里各镜像站差距很大：cargo 那项清华
# 11.28 MB/s、北外 88.72 MB/s，差 8 倍；apt 用清华时 update 要 111s，
# 换北外后 2s。所以交给 chsrc 现场测速挑最快的。
# chsrc 本身也留在镜像里，之后随时可以 chsrc set <目标> 重新测。
if [ "${USE_CN_MIRROR:-1}" = "1" ]; then
  log "安装 chsrc 并按测速切换系统源"
  curl -sSL https://chsrc.run/posix -o /tmp/chsrc-install.sh
  bash /tmp/chsrc-install.sh -d /usr/local/bin
  rm -f /tmp/chsrc-install.sh
  chsrc set ubuntu
fi

log "apt-get update"
apt-get update -qq

# ------------------------------------------------------------ 系统工具
# tzdata 必须单独先来一轮：patch_tzdata 要改的是 postinst 文件，而这个文件
# 要等包解包之后才存在。所以先让 apt 解包它（postinst 会失败，忽略），补丁
# 打上去，再 dpkg --configure -a 把它配置好。顺序反了补丁就是空操作。
log "先处理 tzdata（解包 -> 打补丁 -> 配置）"
apt-get install -y --no-install-recommends tzdata || true
patch_tzdata
dpkg --configure -a

log "安装系统工具"
apt-get install -y --no-install-recommends \
  ca-certificates curl wget gnupg \
  git git-lfs openssh-server \
  tmux htop tree rsync unzip zip less bc jq \
  iproute2 net-tools dnsutils lsof iputils-ping netcat-openbsd psmisc \
  vim nano ncdu \
  python3 python3-pip python3-venv ipython3 \
  build-essential pkg-config libssl-dev

log "安装排查工具"
# 不含 ibverbs-utils / perftest：那是 RDMA 多机训练用的，CPU 机没有 IB。
apt-get install -y --no-install-recommends \
  strace gdb sysstat iotop numactl pciutils

# ---------------------------------------------------------------- Node
log "安装 Node 22 LTS + npm"
curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
apt-get install -y nodejs

# ---------------------------------------------------------------- Rust
# 装到 /usr/local 而不是 /root/.cargo：换任何用户都能用，不绑死 root。
# 这是官方 rust docker 镜像的做法。
log "安装 Rust (minimal + clippy + rustfmt)"
export RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
# static.rust-lang.org 跨境极慢（实测 12 KB/s，清华 6.7 MB/s，差 500 倍）。
# chsrc set rustup 会测速并把 RUSTUP_DIST_SERVER 写进 ~/.bashrc / ~/.zshrc，
# 但那两个文件本次构建的 shell 不会去读（Ubuntu 的 .bashrc 开头就对非交互
# shell return），所以这里把值抓出来 export 一次给当前构建用。
if [ "${USE_CN_MIRROR:-1}" = "1" ] && command -v chsrc >/dev/null; then
  chsrc set rustup || true
  _rs=$(grep -hoE 'RUSTUP_DIST_SERVER="[^"]+"' /root/.bashrc 2>/dev/null | tail -1 | cut -d'"' -f2)
  [ -n "$_rs" ] && export RUSTUP_DIST_SERVER="$_rs" RUSTUP_UPDATE_ROOT="$_rs/rustup"
fi

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --no-modify-path --profile minimal \
      --default-toolchain stable -c clippy -c rustfmt
chmod -R a+rX "$RUSTUP_HOME" "$CARGO_HOME"
cat > /etc/profile.d/rust.sh <<'EOF'
export RUSTUP_HOME=/usr/local/rustup
export CARGO_HOME=/usr/local/cargo
export PATH="$CARGO_HOME/bin:$PATH"
EOF
# ------------------------------------------------------------------ uv
# 用官方独立安装脚本，不用 pip：Ubuntu 24.04 有 PEP 668 保护，
# pip install 需要 --break-system-packages，会污染系统 Python。
log "安装 uv"
curl -LsSf https://astral.sh/uv/install.sh | env UV_INSTALL_DIR=/usr/local/bin sh

# --------------------------------------------------------------- locale
# 裸 ubuntu 只有 C / C.utf8 / POSIX，LANG 是空的，结果 /gfs 里的中文文件名
# 全部显示成八进制转义（实测 测试文件.txt -> ''$'æµ...'）。
# LANG 用 en_US.UTF-8 而不是 zh_CN.UTF-8：报错信息保持英文好搜，同时
# UTF-8 保证中文正常显示，两全。
log "生成 locale"
apt-get install -y --no-install-recommends locales
locale-gen en_US.UTF-8 zh_CN.UTF-8
update-locale LANG=en_US.UTF-8 LC_ALL=

# ------------------------------------------------------------------ zsh
# 注意这一段必须排在"清理 apt 缓存"之前：那一步会 rm -rf
# /var/lib/apt/lists/*，之后再 apt install 会报 Unable to locate package。
log "安装 zsh 并设为默认 shell"
apt-get install -y --no-install-recommends   zsh zsh-autosuggestions zsh-syntax-highlighting

# zshenv 对所有 zsh 会话生效（登录/非登录/交互/非交互）。SSH 里
# `ssh host cmd` 这种非登录非交互调用只读 zshenv，不读 /etc/profile.d，
# 所以 rust 的 PATH 必须放这里。
#
# 用追加而不是覆盖：/etc/zsh/zshenv 是 zsh-common 包的 conffile，整个
# 覆盖会丢掉发行版设的默认 PATH 等内容，包升级时 dpkg 还会来问冲突。
# 平台自己也是往这个文件末尾追加 source /etc/profile.d/gemini.sh。
# 加个标记避免重复运行时叠加多份。
if ! grep -q 'dev-image.sh: BEGIN' /etc/zsh/zshenv 2>/dev/null; then
  cat >> /etc/zsh/zshenv <<'ZSHENVEOF'

# --- dev-image.sh: BEGIN ---
export RUSTUP_HOME=/usr/local/rustup
export CARGO_HOME=/usr/local/cargo
case ":$PATH:" in
  *":$CARGO_HOME/bin:"*) ;;
  *) export PATH="$CARGO_HOME/bin:$PATH" ;;
esac
export LANG=en_US.UTF-8
# --- dev-image.sh: END ---
ZSHENVEOF
fi

# 必须给 root 准备 .zshrc：否则首次启动 zsh 会弹 zsh-newuser-install
# 配置向导，非交互会话（比如脚本、VS Code 的远程 shell）会卡在那里。
cat > /root/.zshrc <<'EOF'
HISTFILE=~/.zsh_history
HISTSIZE=10000
SAVEHIST=10000
setopt SHARE_HISTORY HIST_IGNORE_DUPS HIST_IGNORE_SPACE
setopt AUTO_CD INTERACTIVE_COMMENTS
autoload -Uz compinit && compinit -u
zstyle ':completion:*' menu select
autoload -Uz colors && colors
PROMPT='%F{cyan}%n@%m%f:%F{yellow}%~%f %# '
[ -f /usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh ] &&   . /usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh
[ -f /usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh ] &&   . /usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
alias ll='ls -alF'
EOF

# 装成功了才切默认 shell。先 chsh 再 apt 的话，万一 apt 失败，root 的
# shell 就指向一个不存在的二进制，SSH 直接登不进来。
command -v zsh >/dev/null && chsh -s "$(command -v zsh)" root

# ------------------------------------------------------------ AI CLI
# claude / codex 都由 npm 分发，包里带的是各平台原生二进制（不是 node
# 脚本包装），所以装完 /usr/bin/claude 和 /usr/bin/codex 直接可执行。
log "安装 Claude Code 与 Codex"
if [ "${USE_CN_MIRROR:-1}" = "1" ]; then
  npm config set registry https://registry.npmmirror.com
fi
npm install -g @anthropic-ai/claude-code @openai/codex

# ---------------------------------------------------------------- 收尾
# 另存为是把整个容器复刻成镜像，所以构建过程留下的任何东西都会被烤进去。
# 实测不清理的话有 300 MB 垃圾：npm 缓存 253 MB、apt lists 51 MB，外加
# 一堆构建脚本、日志和 shell 历史。
# --------------------------------------------------- 各工具链换源（测速）
# 放在最后：chsrc 会写 ~/.zshrc、~/.bashrc、~/.config/uv/uv.toml 等文件，
# 必须等这些文件都由本脚本创建完毕之后再跑，否则会被后面的 cat > 覆盖掉。
# rustup 前面已经设过一次（构建时要用），这里再设一次是为了让它落进最终
# 的 .zshrc。
if [ "${USE_CN_MIRROR:-1}" = "1" ] && command -v chsrc >/dev/null; then
  log "按测速切换各工具链的源"
  for dish in pip uv npm cargo rustup; do
    chsrc set "$dish" || echo "  (跳过 $dish)"
  done
fi

log "清理构建痕迹"
# 包管理器缓存
npm cache clean --force 2>/dev/null || true
apt-get clean
rm -rf /var/lib/apt/lists/* /root/.npm /root/.cache
rm -rf /usr/local/rustup/downloads/* /usr/local/rustup/tmp/*
# 跑过 claude/codex/uv 留下的临时物（配置目录下次启动会重建）
# 注意别删 /root/.config/uv：chsrc 把 uv 的镜像配置写在 uv.toml 里
rm -rf /root/.codex/tmp /root/.zcompdump /root/.config/uv/*.bak
# 平台在容器启动时写的 SSH 状态标记，下次启动会重写，别带进镜像
rm -f /root/.ssh/error
# shell 历史里全是构建过程的命令
rm -f /root/.bash_history /root/.zsh_history
# 日志清零而不是删除，保留文件本身与权限
find /var/log -type f -exec truncate -s 0 {} + 2>/dev/null || true

log "完成"
