#!/usr/bin/env bash
# 开发机镜像构建脚本（CPU 版）
# 在 ubuntu:24.04 的开发环境里跑，完事用平台「另存为」存成镜像，
# tag 带本仓库 commit sha 以便回溯。
# 不装 torch/CUDA：CPU 机用不上，会把镜像从 2 GB 撑到 24 GB。

set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

log() { echo "[$(date +%H:%M:%S)] == $* =="; }

# /etc/localtime 是平台 bind mount 进来的，tzdata 的 postinst 用 mv -f
# 覆盖它必然 EBUSY，一失败就连坐掉整条 python3 依赖链。把那句 mv 改成
# no-op（宿主机时区本来就是我们要的）。dpkg 的 path-exclude 无效：
# 动手的是包自己的 postinst，不是解包。
patch_tzdata() {
  local f=/var/lib/dpkg/info/tzdata.postinst
  [ -f "$f" ] || return 0
  # 不用反向引用： 经多层转义会变成字面 0x01 字节。替换成内建 :
  sed -i 's|mv -f "$DPKG_ROOT/etc/localtime.dpkg-new" "$DPKG_ROOT/etc/localtime"|:|' "$f"
  # 修掉旧版补丁留下的损坏行
  sed -i 's|^.rm -f "$DPKG_ROOT/etc/localtime.dpkg-new"$|:|' "$f"
}

# 裸镜像没有 ca-certificates，校验不了 TLS，所以必须先用默认 http 源装它
# 再换 https 国内源，反过来会死锁（所有包 "Unable to locate package"）。
# curl 同样要在这步装，后面装 chsrc 要用。
log "先用默认源装 ca-certificates + curl（换源和装 chsrc 的前提）"
apt-get update -qq
apt-get install -y --no-install-recommends ca-certificates curl || true
patch_tzdata
dpkg --configure -a

# 不写死镜像站：实测 cargo 清华 11 MB/s vs 北外 89 MB/s，差 8 倍，
# 且随时间变化。交给 chsrc 现场测速。chsrc 也留在镜像里备用。
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
# 不含 ibverbs-utils/perftest：那是多机 GPU 训练的事
apt-get install -y --no-install-recommends \
  strace gdb sysstat iotop numactl pciutils

# ---------------------------------------------------------------- Node
log "安装 Node 22 LTS + npm"
curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
apt-get install -y nodejs

# ---------------------------------------------------------------- Rust
# 装到 /usr/local 而非 /root/.cargo，不绑死 root（官方 rust 镜像的做法）
log "安装 Rust (minimal + clippy + rustfmt)"
export RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
# static.rust-lang.org 跨境 12 KB/s，必须换源。chsrc 把设置写进
# ~/.bashrc（非交互 shell 不读），所以抓出值 export 给本次构建。
if [ "${USE_CN_MIRROR:-1}" = "1" ] && command -v chsrc >/dev/null; then
  chsrc set rustup || true
  _rs=$(grep -hoE 'RUSTUP_DIST_SERVER="[^"]+"' /root/.bashrc 2>/dev/null | tail -1 | cut -d'"' -f2)
  [ -n "$_rs" ] && export RUSTUP_DIST_SERVER="$_rs" RUSTUP_UPDATE_ROOT="$_rs/rustup"
fi

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --no-modify-path --profile minimal \
      --default-toolchain stable -c clippy -c rustfmt
chmod -R a+rX "$RUSTUP_HOME" "$CARGO_HOME"
# 软链进 /usr/local/bin：平台启动时 source 的 gemini.sh 会硬性重置 PATH
# （且排在我们之后），往 PATH 追加一律无效。/usr/local/bin 在它写死的
# PATH 里，rustup 按 argv[0] 分发，软链照常工作。
for _f in "$CARGO_HOME"/bin/*; do
  [ -e "$_f" ] && ln -sf "$_f" "/usr/local/bin/$(basename "$_f")"
done

cat > /etc/profile.d/rust.sh <<'EOF'
export RUSTUP_HOME=/usr/local/rustup
export CARGO_HOME=/usr/local/cargo
export PATH="$CARGO_HOME/bin:$PATH"
EOF
# ------------------------------------------------------------------ uv
# 用官方脚本不用 pip：24.04 有 PEP 668，pip 装要 --break-system-packages
log "安装 uv"
curl -LsSf https://astral.sh/uv/install.sh | env UV_INSTALL_DIR=/usr/local/bin sh

# 裸镜像 LANG 为空，/gfs 里中文文件名会显示成八进制转义。
# 用 en_US.UTF-8：报错保持英文好搜，UTF-8 保证中文正常。
log "生成 locale"
apt-get install -y --no-install-recommends locales
locale-gen en_US.UTF-8 zh_CN.UTF-8
update-locale LANG=en_US.UTF-8 LC_ALL=

# ------------------------------------------------------------------ zsh
# 注意这一段必须排在"清理 apt 缓存"之前：那一步会 rm -rf
# /var/lib/apt/lists/*，之后再 apt install 会报 Unable to locate package。
log "安装 zsh 并设为默认 shell"
apt-get install -y --no-install-recommends   zsh zsh-autosuggestions zsh-syntax-highlighting

# zshenv 对所有 zsh 调用生效（ssh host cmd 只读它，不读 /etc/profile.d）。
# 用追加不用覆盖：它是 zsh-common 的 conffile。标记用于幂等。
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

# 必须有 .zshrc，否则首次启动 zsh 弹 newuser 向导，非交互会话会卡死
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

# 装成功才切：反过来万一 apt 失败，root 的 shell 指向不存在的二进制，
# SSH 直接登不进来
command -v zsh >/dev/null && chsh -s "$(command -v zsh)" root

# claude / codex 走 npm，包里是各平台原生二进制，装完直接可执行
log "安装 Claude Code 与 Codex"
if [ "${USE_CN_MIRROR:-1}" = "1" ]; then
  npm config set registry https://registry.npmmirror.com
fi
npm install -g @anthropic-ai/claude-code @openai/codex

# 另存为会把整个容器复刻成镜像，构建痕迹都会被烤进去。
# 实测不清理有 300 MB 垃圾（npm 缓存 253 MB + apt lists 51 MB + 日志）。
# 放最后：chsrc 会写 .zshrc/.bashrc/uv.toml，必须等这些文件建好再跑，
# 否则被后面的 cat > 覆盖。rustup 重设一次是为了落进最终的 .zshrc。
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
# claude/codex 的临时物
# 别删整个 /root/.config/uv：chsrc 的镜像配置在 uv.toml 里
rm -rf /root/.codex/tmp /root/.zcompdump /root/.config/uv/*.bak
# 平台写的 SSH 状态标记，下次启动会重写
rm -f /root/.ssh/error
# 历史里全是构建命令
rm -f /root/.bash_history /root/.zsh_history
# 清零而非删除，保留文件与权限
find /var/log -type f -exec truncate -s 0 {} + 2>/dev/null || true

log "完成"
