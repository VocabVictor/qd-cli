# 测试说明

## 自动化测试层级

```powershell
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --test performance -- --ignored --nocapture
```

覆盖范围：

- 12 个单元测试：API 前缀、业务错误脱敏、配置校验、参数解析、任务终态、百分位、ID 注入防护、敏感字段递归脱敏和 Windows DPAPI 往返。
- 1 个 CLI 集成测试：独立进程账号密码登录、RSA 解密核对、DPAPI 会话、401 自动 refresh、`whoami`、项目查询、有界并发批量取消，以及平台不返回 jobId 时的提交、任务反查与等待流程。
- 1 个显式性能测试：512 次 GET、并发 32，并采样子进程工作集。

2026-07-22 本机 Debug 性能测试结果：

```text
requests=512
concurrency=32
throughput=1020.1 req/s
p95=56.56 ms
peak working set=11.49 MiB
```

测试服务人为加入了每次请求 20 ms 延迟，以验证请求确实并发执行。性能阈值要求吞吐量高于 100 req/s、峰值工作集低于 96 MiB。

## 真实平台测试

真实平台测试不在脚本或参数中保存密码。先交互登录：

```powershell
.\bin\qd.exe login --username <账号>
```

登录后依次验证身份、项目、资源、镜像、任务列表和只读性能：

```powershell
.\bin\qd.exe whoami
.\bin\qd.exe project list
.\bin\qd.exe resource groups
.\bin\qd.exe image list
.\bin\qd.exe job list 734704873928396800
.\bin\qd.exe bench /project/list --requests 200 --warmup 5 --concurrency 32
```

### 2026-07-22 实测记录

- `whoami`、项目详情、资源组、任务列表和开发环境详情均返回成功。
- 真实 API 基准：200 次请求、并发 32、0 失败，139.56 req/s；p50 177.25 ms、p95 537.25 ms、p99 596.50 ms。
- 原开发环境位于无可用节点的资源组，启动请求被平台拒绝；未留下运行中的开发环境。
- 在 `yukd-exp` 通过资源预检后，真实提交 1 张 NVIDIA H800 80GB 的 PyTorch 推理任务。任务最终 `SUCCEEDED`，日志包含 `QD_GPU_OK NVIDIA H800 80GB`，证明 CUDA 张量乘法确实执行。
- GPU 测试任务随后已删除，并确认不再出现在项目任务列表中。
- 实测发现并修复：Windows PowerShell 中文乱码、镜像列表缺少 `source/keyWords` 参数、超长 projectId 必须按字符串提交、创建接口成功但不返回 jobId、任务详情中 URL/token 脱敏、refresh token 轮换保存。
- 当前账号对 `image list --source current` 返回权限不足；`--source mine` 可用但列表为空。已保留该平台权限边界，不伪造结果。
