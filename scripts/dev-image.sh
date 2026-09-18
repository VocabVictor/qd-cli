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

# 默认源是 archive.ubuntu.com，实测 apt-get update 要 77s，清华镜像 4s。
# 设 USE_CN_MIRROR=0 可以保留官方源。
if [ "${USE_CN_MIRROR:-1}" = "1" ]; then
  log "切换 apt 源到清华镜像"
  sed -i 's|http://archive.ubuntu.com/ubuntu|https://mirrors.tuna.tsinghua.edu.cn/ubuntu|g; s|http://security.ubuntu.com/ubuntu|https://mirrors.tuna.tsinghua.edu.cn/ubuntu|g'     /etc/apt/sources.list.d/ubuntu.sources /etc/apt/sources.list 2>/dev/null || true
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
# static.rust-lang.org 跨境很慢（实测单这一步就要好几分钟）。走清华镜像。
if [ "${USE_CN_MIRROR:-1}" = "1" ]; then
  export RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup
  export RUSTUP_UPDATE_ROOT=https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup
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
# cargo 拉 crates 默认走 crates.io，同样跨境慢
if [ "${USE_CN_MIRROR:-1}" = "1" ]; then
  cat > "$CARGO_HOME/config.toml" <<'EOF'
[source.crates-io]
replace-with = "tuna"

[source.tuna]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
EOF
fi
chmod 0644 /etc/profile.d/rust.sh

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

# ------------------------------------------------------- pip / uv 镜像
# apt/npm/cargo/rustup 都换了国内源，pip 也得换，否则会重演 rustup 那个
# 12 KB/s 的惨案。uv 下载 Python 解释器走的是 GitHub release，另有变量。
if [ "${USE_CN_MIRROR:-1}" = "1" ]; then
  log "配置 pip / uv 镜像"
  cat > /etc/pip.conf <<'PIPEOF'
[global]
index-url = https://pypi.tuna.tsinghua.edu.cn/simple
trusted-host = pypi.tuna.tsinghua.edu.cn
PIPEOF
  cat > /etc/profile.d/mirrors.sh <<'MIREOF'
export UV_DEFAULT_INDEX=https://pypi.tuna.tsinghua.edu.cn/simple
export UV_PYTHON_INSTALL_MIRROR=https://mirrors.tuna.tsinghua.edu.cn/github-release/astral-sh/python-build-standalone/
MIREOF
  chmod 0644 /etc/profile.d/mirrors.sh
fi

# ------------------------------------------------------------------ zsh
# 注意这一段必须排在"清理 apt 缓存"之前：那一步会 rm -rf
# /var/lib/apt/lists/*，之后再 apt install 会报 Unable to locate package。
log "安装 zsh 并设为默认 shell"
apt-get install -y --no-install-recommends   zsh zsh-autosuggestions zsh-syntax-highlighting

# zshenv 对所有 zsh 会话生效（登录/非登录/交互/非交互），
# /etc/profile.d/rust.sh 只在登录 shell 里被 source，不够用。
cat > /etc/zsh/zshenv <<'EOF'
export RUSTUP_HOME=/usr/local/rustup
export CARGO_HOME=/usr/local/cargo
case ":$PATH:" in
  *":$CARGO_HOME/bin:"*) ;;
  *) export PATH="$CARGO_HOME/bin:$PATH" ;;
esac
export LANG=en_US.UTF-8
export UV_DEFAULT_INDEX=https://pypi.tuna.tsinghua.edu.cn/simple
export UV_PYTHON_INSTALL_MIRROR=https://mirrors.tuna.tsinghua.edu.cn/github-release/astral-sh/python-build-standalone/
EOF

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
log "清理 apt 缓存"
apt-get clean
rm -rf /var/lib/apt/lists/*

log "完成"
