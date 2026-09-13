# qudongctl (`qd`)

把 GPU 训练平台的 Web API 封装成接近 Slurm 使用习惯的 Rust CLI。当前版本覆盖账号密码登录、项目、训练任务、开发环境、资源组、镜像和通用 API 调用。

设计重点：

- 单线程 Tokio 事件循环承载大量并发网络请求，运行时线程和栈内存很少。
- 一个 `reqwest::Client` 全程复用连接池；GET 请求支持超时、限次退避重试。
- 批量取消、删除、启动、停止、查询采用有界并发，默认 32，可调到 1–1024。
- 不接受明文密码命令行参数。交互时隐藏输入，自动化使用标准输入。
- Access/refresh token 和自动登录凭据通过 Windows DPAPI 加密，只能在当前 Windows 用户和本机环境中解密；`--no-remember` 可禁用凭据保存。
- 项目、任务、资源组、闲置资源和镜像列表默认输出对齐的中文表格；加 `--compact` 输出单行 JSON，适合 PowerShell、`jq` 和脚本处理。

## 构建

本机已经安装 Rust 1.97.1 和 Visual Studio 2022 C++ Build Tools。PowerShell 中运行：

```powershell
cd C:\Users\vmuser\Desktop\qudongctl
.\build.ps1
```

低内存编译：

```powershell
.\build.ps1 -Jobs 1
```

生成文件为 `bin\qd.exe`。Release 配置启用了 thin LTO、符号剥离和 `panic=abort`。

## 首次使用

```powershell
# 默认隐藏输入密码
.\bin\qd.exe login --username 你的账号

# 自动化：密码走 stdin，不进入命令历史或进程参数
$secret | .\bin\qd.exe login --username 你的账号 --password-stdin

.\bin\qd.exe whoami
.\bin\qd.exe config show
```

首次使用先设置平台地址与工作空间：

```powershell
qd config set base-url https://平台地址:端口
qd config set space-id 工作空间ID
qd config set concurrency 64
```

## Slurm 风格操作

对应关系：

| Slurm | qd |
|---|---|
| `squeue` | `qd job list PROJECT_ID` |
| `scontrol show job` | `qd job show JOB_ID` |
| `sbatch` | `qd job submit --file job.json` |
| `scancel` | `qd job cancel JOB_ID...` |
| 等待完成 | `qd job wait JOB_ID...` |

例子：

```powershell
qd project list
qd job list
qd dev list
qd resource groups
qd idle --available-only
qd idle --gpu-only
qd resource nodes --gpu-type physical --gpu-model "NVIDIA H800 80GB" --min-gpu 1
qd resource node <节点IP>
qd image list --source mine

# 列表命令默认是表格；脚本读取时改为紧凑 JSON
qd idle --compact
```

`qd idle` 的 GPU 列给两个数，别混用：

- **整组** = 该资源组全部节点空闲整卡之和（`idle.gpuWholeCardsFree`）
- **单实例上限** = 一个实例最多能申请到的卡数（`idle.gpus[].count`），受最好的
  那一个节点限制（`idle.gpuMaxOnOneNode`）

两者不等说明空闲卡分散在多个节点上。例如「整组 8 / 单实例 4，分布在 2 个节点」
意味着提单实例 8 卡会一直排队调度不上，要写成 2 实例 × 4 卡的多节点作业才吃得满。

```powershell
qd job template > job.json
# 编辑 projectId、rsgroupId、imageId、GPU 型号和启动命令后提交；超长 ID 保持 JSON 字符串
qd job submit --file job.json --wait

# 同时取消多个任务；并发数有界，不会为每个 ID 常驻一个大线程
qd job cancel 123 456 789 --concurrency 32

qd dev show JOBENV_ID
qd dev stop JOBENV_ID
qd dev start JOBENV_ID
```

项目、任务、开发环境和镜像列表默认只查询当前账号自己的内容。项目相关命令可用 `--scope shared`、`--scope public` 或 `--scope all` 显式扩大范围；`qd job list PROJECT_ID` 用于查看指定项目的任务。

### 自动保活

`qd` 会在 API 返回 401 时自动刷新 access token，并把平台轮换后的 refresh token 继续用 DPAPI 加密保存。若 refresh token 也失效，会读取 DPAPI 凭据自动重新登录。若希望长时间不使用 CLI 时也保持登录，可安装低频 Windows 计划任务：

```powershell
qd keepalive install --minutes 30
qd keepalive status
```

计划任务每 30 分钟启动一次短进程，刷新并验证会话后立即退出，不会常驻内存。卸载命令：

```powershell
qd keepalive remove
```

## 网页控制台

`qd web` 在本机起一个内嵌网页控制台（单二进制、无外部依赖），复用 CLI 的登录态与自动刷新逻辑：

```powershell
qd web            # 默认 http://127.0.0.1:7717/
qd web --open     # 启动并自动打开浏览器
qd web --port 8080
```

多工作空间自动合并：所有列表跨用户全部空间聚合显示（带空间列），详情/日志/操作自动路由到对应空间。
覆盖 CLI 全部能力：概览（各资源组空闲整卡/CPU/内存）、作业（列表、详情、日志跟随、
GPU/CPU/内存监控曲线、提交、取消、删除）、开发环境（启停、开发者工具、SSH 开关、网页终端）、
项目、节点、镜像、远程执行（等价 `job/dev exec`，捕获输出与退出码）、API 调试（透传 + bench）、
配置与保活管理。未登录时网页内可直接登录（DPAPI 记住凭据）。

安全说明：服务默认只绑 `127.0.0.1`，并拒绝携带非本机 Origin 的跨站请求；
`--host` 绑其它地址会让同网段的人以你的身份操作平台，慎用。

## 未封装接口

通用入口仍会自动处理 Bearer token、`spaceid`、服务前缀和 JSON：

```powershell
qd api GET /project/list --file query.json
qd api GET /user/job/event/list --service tool --file query.json
qd api POST /job/new --file job.json
```

`core` 路径会遵循 Web 端相同的路由规则；`auth`、`tool`、`absolute` 可通过 `--service` 显式选择。接口映射通过 `--service` 与 `--full` 自行探索。

自动化测试、性能指标和真实平台验收步骤见 [TESTING.md](TESTING.md)。

## 配置与安全

- 普通配置：`%APPDATA%\qd\config.json`
- 登录状态：`%LOCALAPPDATA%\qd\session.bin`（DPAPI 密文）
- 自动登录凭据：`%LOCALAPPDATA%\qd\credentials.bin`（DPAPI 密文，不会由 `config show` 输出）
- 临时 token 也可由 `QD_TOKEN` 注入；程序会递归脱敏 token、密码、认证字段和带凭据的终端 URL。
- 平台刷新登录状态时若轮换 refresh token，`qd` 会同步更新 DPAPI 密文会话。
- `qd logout` 会同时删除会话和自动登录凭据。
- 若部署使用自签名或 IP 证书，默认 `insecure_tls=true`。更换为可信证书后执行 `qd config set insecure-tls false`。
