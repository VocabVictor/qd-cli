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

# ---------------------------------------------------------------- apt 源
# 默认源是 archive.ubuntu.com，实测 apt-get update 要 77s。换清华镜像后
# 明显更快。设 USE_CN_MIRROR=0 可以保留官方源。
if [ "${USE_CN_MIRROR:-1}" = "1" ]; then
  log "切换 apt 源到清华镜像"
  sed -i 's|http://archive.ubuntu.com/ubuntu|https://mirrors.tuna.tsinghua.edu.cn/ubuntu|g; s|http://security.ubuntu.com/ubuntu|https://mirrors.tuna.tsinghua.edu.cn/ubuntu|g' \
    /etc/apt/sources.list.d/ubuntu.sources /etc/apt/sources.list 2>/dev/null || true
fi

log "apt-get update"
apt-get update -qq

# ------------------------------------------------------------ 系统工具
log "安装系统工具"
apt-get install -y --no-install-recommends \
  ca-certificates curl wget gnupg \
  git git-lfs openssh-server \
  tmux htop tree rsync unzip zip less bc jq \
  iproute2 net-tools dnsutils lsof \
  python3 python3-pip python3-venv \
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
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --no-modify-path --profile minimal \
      --default-toolchain stable -c clippy -c rustfmt
chmod -R a+rX "$RUSTUP_HOME" "$CARGO_HOME"
cat > /etc/profile.d/rust.sh <<'EOF'
export RUSTUP_HOME=/usr/local/rustup
export CARGO_HOME=/usr/local/cargo
export PATH="$CARGO_HOME/bin:$PATH"
EOF
chmod 0644 /etc/profile.d/rust.sh

# ------------------------------------------------------------------ uv
# 用官方独立安装脚本，不用 pip：Ubuntu 24.04 有 PEP 668 保护，
# pip install 需要 --break-system-packages，会污染系统 Python。
log "安装 uv"
curl -LsSf https://astral.sh/uv/install.sh | env UV_INSTALL_DIR=/usr/local/bin sh

# ---------------------------------------------------------------- 收尾
log "清理 apt 缓存"
apt-get clean
rm -rf /var/lib/apt/lists/*

log "完成"
