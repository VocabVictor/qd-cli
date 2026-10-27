// qd web 浏览器端到端测试：本地起模拟 GPU 平台 + 真实 qd web（公开绑定、两种邀请码），
// 用 Playwright 走完整交互：注册登录、概览热力图、提交作业（含配额拦截）、性能监控、运行日志、
// 网页终端（经 qd 反向代理的 WebSocket 回显）、取消作业、权限边界、管理员分配配额。
// 全程隔离：临时配置目录 + 模拟平台 + 假 token，不碰真实集群。
//
// 用法: node e2e/ui.mjs /path/to/qd [port]
// 环境变量: PW_CHANNEL=chrome（系统 Chrome）或 PW_CHROMIUM=/path/to/chrome（浏览器可执行文件）
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createPlatform } from "./mock-platform.mjs";

if (!process.argv[2]) { console.error("用法: node e2e/ui.mjs /path/to/qd [port]"); process.exit(2); }
const bin = resolve(process.argv[2]);
const port = Number(process.argv[3] || 28172);
const base = `http://127.0.0.1:${port}`;
const shots = join(dirname(fileURLToPath(import.meta.url)), "screenshots");
mkdirSync(shots, { recursive: true });

const platform = createPlatform();
const platformUrl = await platform.listen(0);
const work = mkdtempSync(join(tmpdir(), "qd-ui-"));
const USER_CODE = "ui-user-invite-" + Math.random().toString(36).slice(2, 10);
const ADMIN_CODE = "ui-admin-invite-" + Math.random().toString(36).slice(2, 10);
const env = {
  ...process.env,
  XDG_CONFIG_HOME: join(work, "config"), XDG_STATE_HOME: join(work, "state"), HOME: join(work, "home"),
  APPDATA: join(work, "config"), LOCALAPPDATA: join(work, "state"),
  QD_TOKEN: "fake-token-for-ui-test", QD_USERNAME: "platform-bot",
  QD_WEB_INVITE_CODE: USER_CODE, QD_WEB_ADMIN_INVITE_CODE: ADMIN_CODE,
};
for (const d of [env.XDG_CONFIG_HOME, env.XDG_STATE_HOME, env.HOME]) mkdirSync(d, { recursive: true });

const results = [];
const check = (desc, ok, extra = "") => {
  results.push({ desc, ok: !!ok });
  console.log(`${ok ? "PASS" : "FAIL"}: ${desc}${ok || !extra ? "" : " — " + extra}`);
};
const step = async (desc, fn) => {
  try { await fn(); } catch (err) { check(desc, false, String(err?.message || err).split("\n")[0]); }
};

async function waitUp() {
  for (let i = 0; i < 60; i++) {
    try { await fetch(base + "/"); return; } catch { await new Promise((r) => setTimeout(r, 200)); }
  }
  throw new Error("qd web 没能启动");
}

const server = spawn(bin, ["--base-url", platformUrl, "web", "--host", "0.0.0.0", "--port", String(port)], { env, stdio: ["ignore", "pipe", "pipe"] });
let serverLog = "";
server.stdout.on("data", (d) => (serverLog += d));
server.stderr.on("data", (d) => (serverLog += d));

const launch = {};
if (process.env.PW_CHANNEL) launch.channel = process.env.PW_CHANNEL;
if (process.env.PW_CHROMIUM) launch.executablePath = process.env.PW_CHROMIUM;

let browser;
const jsErrors = [];
try {
  await waitUp();
  browser = await chromium.launch({ headless: true, ...launch });
  const open = async () => {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2, locale: "zh-CN" });
    const page = await ctx.newPage();
    page.on("pageerror", (e) => jsErrors.push(String(e)));
    page.on("console", (m) => { if (m.type() === "error" && !/Failed to load resource/.test(m.text())) jsErrors.push(m.text()); });
    await page.goto(base + "/", { waitUntil: "networkidle" });
    return { ctx, page };
  };
  const toasts = (page) => page.locator("[role=status] > div").allInnerTexts().then((t) => t.join(" | "));
  const waitToast = async (page, re, ms = 6000) => {
    const end = Date.now() + ms;
    while (Date.now() < end) { if (re.test(await toasts(page))) return true; await page.waitForTimeout(150); }
    return false;
  };
  const shot = (page, name, full = false) => page.screenshot({ path: join(shots, name), fullPage: full });
  const register = async (page, user, pass, code) => {
    await page.waitForSelector("#auth-user");
    if (await page.getByRole("tab", { name: "注册" }).getAttribute("aria-pressed") !== "true") await page.getByRole("tab", { name: "注册" }).click();
    await page.fill("#auth-user", user);
    await page.fill("#auth-pass", pass);
    await page.fill("#auth-invite", code);
    await page.click("form button[type=submit]");
    await page.waitForSelector("aside nav", { timeout: 10000 });
  };

  // ---------- 1. 账号墙与管理员注册 ----------
  const A = await open();
  await step("管理员注册", async () => {
    await A.page.waitForSelector("#auth-user");
    check("首次打开显示本机账号墙", await A.page.getByText("需要本机账号才能进入").isVisible());
    check("没有账号时默认在注册页签", await A.page.locator("#auth-invite").isVisible());
    await shot(A.page, "01-auth.png");
    await register(A.page, "boss", "bosspass123", ADMIN_CODE);
    check("管理员能看到「用户与配额」导航", await A.page.getByRole("button", { name: "用户与配额" }).isVisible());
    check("侧栏没有红黄绿装饰点", (await A.page.locator("aside .rounded-full.w-3.h-3").count()) === 0);
  });

  // ---------- 2. 概览页：热力图是主体 ----------
  await step("概览页", async () => {
    await A.page.waitForSelector("text=节点可用性");
    await A.page.waitForSelector("button[aria-label*='空闲']", { timeout: 10000 });
    const gpuNodes = platform.nodes.filter((n) => n.gpuTotal > 0).length;
    const squares = await A.page.locator("button[aria-label*='空闲']").count();
    check(`热力图每个 GPU 节点一个方块（${gpuNodes} 个）`, squares === gpuNodes, `实际 ${squares}`);
    const freeTotal = platform.nodes.reduce((a, n) => a + n.gpuLeft, 0);
    const cardTotal = platform.nodes.reduce((a, n) => a + n.gpuTotal, 0);
    const summary = await A.page.locator("section", { hasText: "空闲 GPU" }).first().innerText();
    check(`资源摘要显示 ${freeTotal} / ${cardTotal} 卡空闲`, summary.replace(/\s+/g, " ").includes(`${freeTotal} / ${cardTotal} 卡`), summary);
    check("资源组表列出 A100 训练池", await A.page.locator("table").getByText("A100 训练池").isVisible());
    check("纯 CPU 资源组默认收起", !(await A.page.locator("table").getByText("CPU 通用池").isVisible()));
    await A.page.getByRole("button", { name: /显示 1 个纯 CPU 资源组/ }).click();
    check("点开后显示纯 CPU 资源组", await A.page.locator("table").getByText("CPU 通用池").isVisible());
    check("概览列出我的作业 llama-sft-baseline", await A.page.getByText("llama-sft-baseline").first().isVisible());
    await A.page.mouse.move(0, 0);
    await shot(A.page, "02-overview.png", true);
  });

  // ---------- 3. 成员注册：配额 0 不能提交 GPU 作业 ----------
  const B = await open();
  await step("成员注册与零配额拦截", async () => {
    await register(B.page, "alice", "alicepass1", USER_CODE);
    check("成员看不到「用户与配额」导航", !(await B.page.getByRole("button", { name: "用户与配额" }).isVisible()));
    check("成员侧栏显示 0 / 0 卡", await B.page.locator("aside").getByText("GPU 已用 0 / 0 卡").isVisible());
    await B.page.getByRole("button", { name: "提交作业" }).first().click();
    await B.page.waitForSelector("#cf-name");
    await B.page.selectOption("#cf-repo", "r1");
    await B.page.waitForFunction(() => document.querySelector("select[aria-label='镜像版本'] option[value='11']"));
    const submit = B.page.locator("[role=dialog] button", { hasText: "提交作业" });
    check("零配额时提交按钮不可用", await submit.isDisabled());
    check("底栏说明配额不足", /GPU 配额不足/.test(await B.page.locator("[role=dialog]").innerText()));
    await B.page.locator(".segmented button", { hasText: "CPU" }).first().click();
    check("切到 CPU 后可以提交", await submit.isEnabled());
    await shot(B.page, "03-submit-no-quota.png");
    await B.page.keyboard.press("Escape");
  });

  // ---------- 4. 管理员分配配额 ----------
  await step("管理员分配配额", async () => {
    await A.page.getByRole("button", { name: "用户与配额" }).click();
    await A.page.waitForSelector("text=alice");
    const row = A.page.locator(".card", { hasText: "alice" }).first();
    await row.getByLabel("总配额（全部空间） 上限").fill("4");
    await row.getByLabel("训练空间 上限", { exact: true }).fill("4");
    await A.page.waitForSelector("text=A100 训练池");
    await row.getByLabel(/A100 训练池.* 上限/).fill("2");
    await row.getByRole("button", { name: "保存配额" }).click();
    check("配额保存成功", await waitToast(A.page, /配额已保存/));
    await shot(A.page, "04-admin.png", true);
  });

  // ---------- 5. 成员从热力图进入提交，提交 1 卡作业 ----------
  let jobId = "";
  await step("成员提交作业", async () => {
    await B.page.reload({ waitUntil: "networkidle" });
    await B.page.waitForSelector("aside nav");
    check("成员看到新配额 0 / 4 卡", await B.page.locator("aside").getByText("GPU 已用 0 / 4 卡").isVisible());
    await B.page.waitForSelector("button[aria-label^='g1-node-01']");
    await B.page.click("button[aria-label^='g1-node-01']");
    await B.page.waitForSelector("#cf-group");
    check("从热力图进入时预选该资源组", (await B.page.inputValue("#cf-group")) === "g1");
    await B.page.fill("#cf-name", "alice-finetune");
    await B.page.selectOption("#cf-repo", "r1");
    await B.page.waitForFunction(() => document.querySelector("select[aria-label='镜像版本'] option[value='11']"));
    await B.page.fill("input[aria-label='GPU 卡数']", "3");
    const submit = B.page.locator("[role=dialog] button", { hasText: "提交作业" });
    check("超过资源组配额（2 卡）时不能提交", await submit.isDisabled());
    await B.page.fill("input[aria-label='GPU 卡数']", "1");
    check("底栏摘要反映规格", /1 × A100-80G/.test(await B.page.getByTestId("job-summary").innerText()));
    await shot(B.page, "05-submit.png");
    await submit.click();
    check("提交成功提示", await waitToast(B.page, /已提交作业 9\d+/));
    const sent = platform.submitted.at(-1) || {};
    jobId = platform.jobs[0].jobId;
    check("平台收到的作业：g1、1 卡、镜像 11", sent.rsgroupId === "g1" && sent.taskroles?.[0]?.gpu === 1 && sent.imageId === 11, JSON.stringify(sent).slice(0, 200));
    await B.page.reload({ waitUntil: "networkidle" });
    await B.page.waitForSelector("aside nav");
    check("提交后侧栏占用变为 1 / 4 卡", await B.page.locator("aside").getByText("GPU 已用 1 / 4 卡").isVisible());
  });

  // ---------- 6. 作业页：归属、监控、日志、终端 ----------
  await step("作业监控与终端", async () => {
    await B.page.getByRole("button", { name: "作业", exact: true }).click();
    await B.page.waitForSelector("text=alice-finetune");
    const others = B.page.locator("tr[data-job='880001']");
    check("作业页默认只列运行中的作业", (await B.page.locator("tr[data-job='880002']").count()) === 0);
    check("别人的作业：取消按钮不可用", await others.getByRole("button", { name: "取消" }).isDisabled());
    check("别人的作业：终端按钮不可用", await others.getByRole("button", { name: "终端" }).isDisabled());
    const mine = B.page.locator(`tr[data-job='${jobId}']`);
    check("自己的作业标注「我提交的」", await mine.getByText("我提交的").isVisible());
    await shot(B.page, "06-jobs.png", true);

    await mine.getByText("alice-finetune").first().click();
    await B.page.waitForSelector("[role=dialog] [data-metric]", { timeout: 10000 });
    check("性能监控显示 4 项指标", (await B.page.locator("[role=dialog] [data-metric]").count()) === 4);
    check("监控有曲线", (await B.page.locator("[role=dialog] [data-metric] svg path").count()) >= 4);
    await shot(B.page, "07-monitor.png");

    await B.page.getByRole("tab", { name: "运行日志" }).click();
    await B.page.waitForSelector("[data-testid=job-logs]");
    check("运行日志显示平台日志行", /epoch 1 step 10 loss/.test(await B.page.getByTestId("job-logs").innerText()));
    await shot(B.page, "08-logs.png");

    await B.page.getByRole("tab", { name: "网页终端" }).click();
    const frame = B.page.frameLocator("iframe[title='网页终端']");
    await frame.locator("#out").filter({ hasText: `connected pod-${jobId}` }).waitFor({ timeout: 10000 });
    check("终端经 qd 代理连上自己作业的 pod", true);
    // 在自己作业的终端里敲一组真实命令，逐条核对输出（包括状态延续：cd 之后 pwd 要变）
    const run = async (cmd, expect) => {
      await frame.locator("#in").fill(cmd);
      await frame.locator("#in").press("Enter");
      await frame.locator("#out").filter({ hasText: expect }).waitFor({ timeout: 5000 });
    };
    await step("终端命令", async () => {
      await run("hostname", `pod-${jobId}`);
      check("hostname 返回自己作业的 pod", true);
      await run("pwd", "/gemini/code");
      await run("ls", "main.py  configs  data");
      check("pwd / ls 返回工作目录内容", true);
      await run("cd data", "");
      await run("pwd", "/gemini/code/data");
      check("cd 之后 pwd 跟着变（会话状态保持）", true);
      await run("nvidia-smi", "A100-SXM4-80GB");
      check("nvidia-smi 看到 GPU", true);
      await run("python --version", "Python 3.11.9");
      await run("echo 训练开始 ok", "训练开始 ok");
      check("中文参数原样往返", true);
      await run("foo", "bash: foo: command not found");
      check("未知命令返回 shell 报错", true);
      const lines = (await frame.locator("#out").innerText()).split("\n").filter((l) => l.startsWith("$ "));
      check(`终端里共执行 ${lines.length} 条命令`, lines.length >= 9, String(lines.length));
    });
    await shot(B.page, "09-terminal.png");
    // 焦点在终端 iframe 里时 Esc 由终端接收（终端里 Esc 有用途），用关闭按钮退出
    await B.page.getByRole("button", { name: "关闭" }).click();
    check("关闭按钮能关掉终端抽屉", (await B.page.locator("[role=dialog]").count()) === 0);

    await others.getByText("llama-sft-baseline").first().click();
    await B.page.waitForSelector("[role=dialog] [data-metric]");
    check("别人的作业：监控可看", true);
    check("别人的作业：终端页签禁用", await B.page.getByRole("tab", { name: "网页终端" }).isDisabled());
    const forbidden = await B.page.evaluate(async (id) => (await fetch(`/api/terminal-url?kind=job&id=${id}`)).status, "880001");
    check("直接请求别人作业的终端地址返回 403", forbidden === 403, String(forbidden));
    await B.page.keyboard.press("Escape");
    check("Esc 能关掉监控抽屉", (await B.page.locator("[role=dialog]").count()) === 0);
  });

  // ---------- 6b. 作业页：平台原生分页、状态计数、关键字搜索 ----------
  await step("作业分页", async () => {
    const pg = B.page;
    const rows = () => pg.locator("tbody tr[data-job]");
    const ids = async () => (await rows().evaluateAll((els) => els.map((e) => e.getAttribute("data-job"))));
    const act = (j) => (["RUNNING", "CREATED", "WAITING", "STOPPING"].includes(j.status) ? 1 : 0);
    const byTime = [...platform.jobs].sort((a, b) => (act(b) - act(a)) || (b.createTime - a.createTime)).map((j) => j.jobId);
    await pg.getByRole("tab", { name: /全部/ }).click();
    await pg.waitForFunction(() => document.querySelectorAll("tbody tr[data-job]").length === 50, null, { timeout: 10000 });
    check(`「全部」计数等于平台作业总数 ${platform.jobs.length}`, (await pg.getByRole("tab", { name: /全部/ }).innerText()).includes(String(platform.jobs.length)));
    check("第 1 页 50 条，跨项目按平台原生顺序（未结束在前，再按创建时间倒序）", JSON.stringify(await ids()) === JSON.stringify(byTime.slice(0, 50)));
    check("分页条显示总数", (await pg.getByTestId("pager").innerText()).includes(`共 ${platform.jobs.length} 个`));
    const before = platform.listCalls.length;
    await pg.getByRole("button", { name: "下一页" }).click();
    await pg.waitForFunction((first) => document.querySelector("tbody tr[data-job]")?.getAttribute("data-job") === first, byTime[50], { timeout: 10000 });
    check("第 2 页接着第 1 页的顺序", JSON.stringify(await ids()) === JSON.stringify(byTime.slice(50, 100)));
    const lastPage = Math.ceil(platform.jobs.length / 50);
    for (let n = 2; n < lastPage; n++) {
      await pg.getByRole("button", { name: "下一页" }).click();
      await pg.waitForFunction((first) => document.querySelector("tbody tr[data-job]")?.getAttribute("data-job") === first, byTime[n * 50], { timeout: 10000 });
    }
    check("最后一页只剩余下的条数", (await rows().count()) === platform.jobs.length - (lastPage - 1) * 50);
    check("下一页按钮在最后一页禁用", await pg.getByRole("button", { name: "下一页" }).isDisabled());
    const calls = platform.listCalls.slice(before);
    check("翻页时每次向平台只要需要的量（pageSize ≤ 页码 × 50）", calls.length > 0 && calls.every((c) => c.pageSize <= lastPage * 50), JSON.stringify(calls.slice(0, 3)));

    await pg.getByRole("tab", { name: /已停止/ }).click();
    const stopped = platform.jobs.filter((j) => ["STOPPING", "STOPPED", "FAILED"].includes(j.status)).length;
    await pg.waitForFunction(() => document.querySelector("[data-testid=pager]")?.textContent?.includes("第 1–"), null, { timeout: 10000 });
    check(`「已停止」按平台状态筛选，共 ${stopped} 个`, (await pg.getByTestId("pager").innerText()).includes(`共 ${stopped} 个`));
    check("已停止页里没有运行中的作业", (await pg.locator("tbody").innerText()).includes("RUNNING") === false);
    check("状态筛选用平台的 jobStatus 参数", platform.listCalls.some((c) => c.jobStatus === "FAILED"));

    await pg.getByRole("tab", { name: /全部/ }).click();
    await pg.fill("input[aria-label='按作业名搜索']", "sweep-lr-007");
    await pg.waitForFunction(() => document.querySelectorAll("tbody tr[data-job]").length === 1, null, { timeout: 10000 });
    check("关键字搜索交给平台 keyWords，结果只剩 1 条", (await rows().first().innerText()).includes("sweep-lr-007") && platform.listCalls.some((c) => c.keyWords === "sweep-lr-007"));
    await pg.fill("input[aria-label='按作业名搜索']", "");
    await pg.getByRole("tab", { name: /运行中/ }).click();
    await pg.waitForSelector(`tr[data-job='${jobId}']`, { timeout: 10000 });
    await shot(pg, "06b-jobs-paged.png");
  });

  // ---------- 7. 取消自己的作业后释放配额 ----------
  await step("取消作业", async () => {
    B.page.once("dialog", (d) => d.accept());
    await B.page.locator(`tr[data-job='${jobId}']`).getByRole("button", { name: "取消" }).click();
    check("取消成功提示", await waitToast(B.page, /已取消/));
    check("平台上作业已停止", platform.jobs.find((j) => j.jobId === jobId)?.status === "STOPPED");
    await B.page.reload({ waitUntil: "networkidle" });
    await B.page.waitForSelector("aside nav");
    check("取消后配额释放为 0 / 4 卡", await B.page.locator("aside").getByText("GPU 已用 0 / 4 卡").isVisible());
  });

  // ---------- 8. 退出 ----------
  await step("退出", async () => {
    await B.page.getByRole("button", { name: "退出" }).click();
    await B.page.waitForSelector("#auth-user", { timeout: 5000 });
    check("退出后回到账号墙", true);
  });

  check("全程无前端 JS 异常", jsErrors.length === 0, jsErrors.join(" ; "));
} catch (err) {
  check("测试脚本执行完成", false, String(err?.stack || err));
} finally {
  await browser?.close();
  server.kill();
  await platform.close();
  rmSync(work, { recursive: true, force: true });
}

const failed = results.filter((r) => !r.ok).length;
if (failed) console.log("\nqd web 日志:\n" + serverLog.slice(-2000));
console.log(`\n通过 ${results.length - failed} / 失败 ${failed}（截图在 ${shots}）`);
process.exit(failed ? 1 : 0);
