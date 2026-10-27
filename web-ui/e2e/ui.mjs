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
  // 默认登录策略绑定设备指纹：页面外直接 fetch 也要带上同一个 X-QD-Device，否则会被当成换了设备踢下线
  const devices = new Map();
  const deviceOf = (page) => devices.get(page);
  const register = async (page, user, pass, code) => {
    await page.waitForSelector("#auth-user");
    devices.set(page, (await page.getByTestId("device-id").innerText()).trim());
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
    check("管理员能看到「用户与权限」导航", await A.page.getByRole("button", { name: "用户与权限" }).isVisible());
    check("侧栏没有红黄绿装饰点", (await A.page.locator("aside .rounded-full.w-3.h-3").count()) === 0);
  });

  // ---------- 2. 概览页：热力图是主体 ----------
  await step("概览页", async () => {
    await A.page.waitForSelector("text=节点可用性");
    await A.page.waitForSelector("button[aria-label*='空闲']", { timeout: 10000 });
    const gpuNodes = platform.nodes.filter((n) => n.gpuTotal > 0).length;
    check("热力图取节点时走原生分页（pageSize 200）", platform.nodeCalls.some((c) => c.pageSize === 200));
    const squares = await A.page.locator("button[aria-label*='空闲']").count();
    check(`热力图每个 GPU 节点一个方块（${gpuNodes} 个）`, squares === gpuNodes, `实际 ${squares}`);
    const freeTotal = platform.nodes.reduce((a, n) => a + n.gpuLeft, 0);
    const cardTotal = platform.nodes.reduce((a, n) => a + n.gpuTotal, 0);
    const summary = await A.page.locator("section", { hasText: "空闲 GPU" }).first().innerText();
    check(`资源摘要显示 ${freeTotal} / ${cardTotal} 卡空闲`, summary.replace(/\s+/g, " ").includes(`${freeTotal} / ${cardTotal} 卡`), summary);
    check("资源组表列出 A100 训练池", await A.page.locator("table").getByText("A100 训练池").isVisible());
    check("纯 CPU 资源组直接列出，不再折叠", await A.page.locator("table").getByText("CPU 通用池").isVisible());
    check("概览列出我的作业 llama-sft-baseline", await A.page.getByText("llama-sft-baseline").first().isVisible());
    await A.page.mouse.move(0, 0);
    await shot(A.page, "02-overview.png", true);
  });

  // ---------- 3. 成员注册：配额 0 不能提交 GPU 作业 ----------
  const B = await open();
  await step("成员注册与零配额拦截", async () => {
    await register(B.page, "alice", "alicepass1", USER_CODE);
    check("成员看不到「用户与权限」导航", !(await B.page.getByRole("button", { name: "用户与权限" }).isVisible()));
    check("成员侧栏显示 0 / 0 卡", await B.page.locator("aside").getByText("GPU 已用 0 / 0 卡").isVisible());
    await B.page.getByRole("button", { name: "提交作业" }).first().click();
    await B.page.waitForSelector("#cf-name");
    await B.page.selectOption("#cf-repo", "1");
    await B.page.waitForFunction(() => document.querySelector("select[aria-label='镜像版本'] option[value='11']"));
    const submit = B.page.locator("[role=dialog] button", { hasText: "提交作业" });
    check("零配额时提交按钮不可用", await submit.isDisabled());
    check("底栏说明配额不足", /GPU 配额不足/.test(await B.page.locator("[role=dialog]").innerText()));
    await B.page.locator(".segmented button", { hasText: "CPU" }).first().click();
    check("切到 CPU 后可以提交", await submit.isEnabled());
    await shot(B.page, "03-submit-no-quota.png");
    check("成员看不到「存为模板」", !(await B.page.getByRole("button", { name: "存为模板" }).isVisible()));
    await B.page.keyboard.press("Escape");
  });

  await step("管理员保存、套用、删除模板", async () => {
    await A.page.getByRole("button", { name: "提交作业" }).first().click();
    await A.page.waitForSelector("#cf-name");
    await A.page.fill("#cf-name", "tpl-source");
    await A.page.getByRole("button", { name: "存为模板" }).click();
    await A.page.fill("input[aria-label='模板名']", "a100-base");
    await A.page.keyboard.press("Enter");
    check("模板保存成功", await waitToast(A.page, /已保存模板 a100-base/));
    const chip = A.page.locator("[aria-label='作业模板'] [role=listitem]", { hasText: "a100-base" });
    check("模板出现在模板栏", await chip.isVisible());
    await shot(A.page, "03-template.png");
    await A.page.fill("#cf-name", "changed");
    await chip.getByRole("button", { name: "a100-base", exact: true }).click();
    check("套用模板恢复表单", (await A.page.inputValue("#cf-name")) === "tpl-source");
    A.page.once("dialog", (d) => d.accept());
    await chip.hover();
    await A.page.getByRole("button", { name: "删除模板 a100-base" }).click();
    check("删除模板", await waitToast(A.page, /已删除模板 a100-base/));
    check("模板栏随之消失", !(await A.page.getByRole("list", { name: "作业模板" }).isVisible()));
    await A.page.keyboard.press("Escape");
  });

  // ---------- 4. 管理员分配配额 ----------
  await step("管理员分配配额", async () => {
    await A.page.getByRole("button", { name: "用户与权限" }).click();
    await A.page.waitForSelector("[data-user=alice]");
    const row = A.page.locator("[data-user=alice]");
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
    await B.page.selectOption("#cf-repo", "1");
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
    const forbidden = await B.page.evaluate(async ([id, dev]) => (await fetch(`/api/terminal-url?kind=job&id=${id}`, { headers: { "X-QD-Device": dev } })).status, ["880001", deviceOf(B.page)]);
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
    await pg.waitForFunction(() => document.querySelectorAll("tbody tr[data-job]").length === 20, null, { timeout: 10000 });
    check(`「全部」计数等于平台作业总数 ${platform.jobs.length}`, (await pg.getByRole("tab", { name: /全部/ }).innerText()).includes(String(platform.jobs.length)));
    check("第 1 页 20 条，跨项目按平台原生顺序（未结束在前，再按创建时间倒序）", JSON.stringify(await ids()) === JSON.stringify(byTime.slice(0, 20)));
    check("分页条显示总数", (await pg.getByTestId("pager").innerText()).includes(`共 ${platform.jobs.length} 个`));
    const before = platform.listCalls.length;
    await pg.getByRole("button", { name: "下一页" }).click();
    await pg.waitForFunction((first) => document.querySelector("tbody tr[data-job]")?.getAttribute("data-job") === first, byTime[20], { timeout: 10000 });
    check("第 2 页接着第 1 页的顺序", JSON.stringify(await ids()) === JSON.stringify(byTime.slice(20, 40)));
    const lastPage = Math.ceil(platform.jobs.length / 20);
    await pg.getByRole("button", { name: String(lastPage), exact: true }).click();
    await pg.waitForFunction((first) => document.querySelector("tbody tr[data-job]")?.getAttribute("data-job") === first, byTime[(lastPage - 1) * 20], { timeout: 10000 });
    check("点页码直接跳到最后一页", (await pg.locator("[data-testid=pager] [aria-current=page]").innerText()) === String(lastPage));
    await pg.getByRole("button", { name: "1", exact: true }).click();
    await pg.waitForFunction((first) => document.querySelector("tbody tr[data-job]")?.getAttribute("data-job") === first, byTime[0], { timeout: 10000 });
    await pg.getByRole("button", { name: "下一页" }).click();
    await pg.waitForFunction((first) => document.querySelector("tbody tr[data-job]")?.getAttribute("data-job") === first, byTime[20], { timeout: 10000 });
    for (let n = 2; n < lastPage; n++) {
      await pg.getByRole("button", { name: "下一页" }).click();
      await pg.waitForFunction((first) => document.querySelector("tbody tr[data-job]")?.getAttribute("data-job") === first, byTime[n * 20], { timeout: 10000 });
    }
    check("最后一页只剩余下的条数", (await rows().count()) === platform.jobs.length - (lastPage - 1) * 20);
    check("下一页按钮在最后一页禁用", await pg.getByRole("button", { name: "下一页" }).isDisabled());
    const calls = platform.listCalls.slice(before);
    check("翻页时每次向平台只要需要的量（pageSize ≤ 页码 × 20）", calls.length > 0 && calls.every((c) => c.pageSize <= lastPage * 20), JSON.stringify(calls.slice(0, 3)));

    await pg.getByRole("tab", { name: /已停止/ }).click();
    const stopped = platform.jobs.filter((j) => ["STOPPING", "STOPPED", "FAILED"].includes(j.status)).length;
    await pg.waitForFunction(() => document.querySelector("[data-testid=pager]")?.textContent?.includes("第 1–"), null, { timeout: 10000 });
    check(`「已停止」按平台状态筛选，共 ${stopped} 个`, (await pg.getByTestId("pager").innerText()).includes(`共 ${stopped} 个`));
    check("已停止页里没有运行中的作业", (await pg.locator("tbody").innerText()).includes("运行中") === false);
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

  // ---------- 6c. 节点页：集群级原生分页、平台筛选与排序、筛选后汇总 ----------
  await step("节点分页", async () => {
    const pg = B.page;
    await pg.getByRole("button", { name: "节点", exact: true }).click();
    await pg.waitForSelector("tbody tr[data-node]", { timeout: 10000 });
    const total = platform.nodes.length;
    const names = () => pg.locator("tbody tr[data-node]").evaluateAll((els) => els.map((e) => e.getAttribute("data-node")));
    const rank = [...platform.nodes].sort((a, b) => (b.gpuLeft - a.gpuLeft) || a.name.localeCompare(b.name)).map((n) => n.name);
    check(`节点共 ${total} 个，第 1 页 20 个`, (await pg.getByTestId("pager").innerText()).includes(`共 ${total} 个`) && (await names()).length === 20);
    check("默认按空闲卡倒序（平台 sortField=left_gpu_count）", JSON.stringify(await names()) === JSON.stringify(rank.slice(0, 20)));
    await pg.getByRole("button", { name: "下一页" }).click();
    await pg.waitForFunction((first) => document.querySelector("tbody tr[data-node]")?.getAttribute("data-node") === first, rank[20], { timeout: 10000 });
    check("第 2 页接着第 1 页", JSON.stringify(await names()) === JSON.stringify(rank.slice(20, 40)));
    check("翻页时向平台要 pageNum=2", platform.nodeCalls.some((c) => c.pageNum === 2 && c.pageSize === 20));
    await pg.check("text=只看有空闲卡的");
    const free = platform.nodes.filter((n) => n.gpuLeft >= 1);
    await pg.waitForFunction((n) => document.querySelector("[data-testid=pager]")?.textContent?.includes(`共 ${n} 个`), free.length, { timeout: 10000 });
    check(`「只看有空闲卡的」交给平台 leftGpuCount，剩 ${free.length} 个并回到第 1 页`, (await pg.locator("[data-testid=pager] [aria-current=page]").innerText()) === "1" && await pg.getByRole("button", { name: "下一页" }).isDisabled() && platform.nodeCalls.some((c) => c.leftGpuCount === 1));
    const freeGpu = free.reduce((a, n) => a + n.gpuTotal, 0);
    check(`汇总用平台按筛选算出的总量（${freeGpu} 卡）`, (await pg.locator("section[aria-label='筛选范围内的资源汇总']").innerText()).includes(`共 ${freeGpu} 卡`));
    await pg.uncheck("text=只看有空闲卡的");
    await pg.waitForFunction((n) => document.querySelector("[data-testid=pager]")?.textContent?.includes(`共 ${n} 个`), total, { timeout: 10000 });
    await pg.selectOption("select[aria-label='GPU 型号']", "L40S");
    const l40 = platform.nodes.filter((n) => n.gpuModels === "L40S").length;
    await pg.waitForFunction((n) => document.querySelector("[data-testid=pager]")?.textContent?.includes(`共 ${n} 个`), l40, { timeout: 10000 });
    check(`按型号筛选交给平台 gpuModel，剩 ${l40} 个`, platform.nodeCalls.some((c) => c.gpuModel === "L40S"));
    await pg.selectOption("select[aria-label='GPU 型号']", "");
    await pg.selectOption("select[aria-label='资源组']", "g3");
    const g3 = platform.nodes.filter((n) => n.group === "g3").length;
    await pg.waitForFunction((n) => document.querySelector("[data-testid=pager]")?.textContent?.includes(`共 ${n} 个`), g3, { timeout: 10000 });
    check(`按资源组筛选交给平台 rsgroup，剩 ${g3} 个`, platform.nodeCalls.some((c) => c.rsgroup === "g3"));
    await pg.selectOption("select[aria-label='资源组']", "");
    const before = platform.nodeCalls.length;
    await pg.getByRole("columnheader", { name: /CPU 已申请/ }).click();
    await pg.waitForFunction(() => true);
    for (let i = 0; i < 40 && !platform.nodeCalls.slice(before).some((c) => c.sortField === "left_cpu"); i++) await pg.waitForTimeout(100);
    check("点表头排序交给平台 sortField=left_cpu", platform.nodeCalls.slice(before).some((c) => c.sortField === "left_cpu"));
    await shot(pg, "10-nodes.png");
  });

  // ---------- 6d. 镜像页：跨空间原生分页归并、可见性与关键字交给平台 ----------
  await step("镜像分页", async () => {
    const pg = B.page;
    await pg.getByRole("button", { name: "镜像", exact: true }).click();
    await pg.waitForSelector("tbody tr[data-repo]", { timeout: 10000 });
    const ids = () => pg.locator("tbody tr[data-repo]").evaluateAll((els) => els.map((e) => Number(e.getAttribute("data-repo"))));
    const order = [...platform.repos].sort((a, b) => b.imageRepositoryId - a.imageRepositoryId).map((r) => r.imageRepositoryId);
    check(`镜像共 ${platform.repos.length} 个（两个空间之和）`, (await pg.getByTestId("pager").innerText()).includes(`共 ${platform.repos.length} 个`));
    check("第 1 页 20 个，跨空间按创建顺序（id 倒序）归并", JSON.stringify(await ids()) === JSON.stringify(order.slice(0, 20)));
    check("成员看不到镜像创建人列", !(await pg.locator("thead").innerText()).includes("创建人"));
    const leaked = await pg.evaluate(async (dev) => JSON.stringify(await (await fetch("/api/images?size=5", { headers: { "X-QD-Device": dev } })).json()), deviceOf(pg));
    check("后端不给成员返回创建人字段", !/displayName|userName|createUserId/.test(leaked), leaked.slice(0, 200));
    await pg.getByRole("button", { name: "下一页" }).click();
    await pg.waitForFunction((first) => Number(document.querySelector("tbody tr[data-repo]")?.getAttribute("data-repo")) === first, order[20], { timeout: 10000 });
    check("第 2 页接着第 1 页", JSON.stringify(await ids()) === JSON.stringify(order.slice(20, 40)));
    check("每个空间都用原生 pageNum/pageSize 取", ["s1", "s2"].every((sp) => platform.imageCalls.some((c) => c.space === sp && c.pageSize <= 40)));
    const shared = platform.repos.filter((r) => r.accessType === 2).length;
    await pg.getByRole("tab", { name: /空间共享/ }).click();
    await pg.waitForFunction((n) => document.querySelector("[data-testid=pager]")?.textContent?.includes(`共 ${n} 个`), shared, { timeout: 10000 });
    check(`「空间共享」交给平台 source=shared，共 ${shared} 个`, platform.imageCalls.some((c) => c.source === "shared"));
    check("可见性计数来自平台", (await pg.getByRole("tab", { name: /空间共享/ }).innerText()).includes(String(shared)));
    const pub = platform.repos.filter((r) => r.accessType === 3).length;
    check(`「公开」是全局列表，不重复累加（${pub} 个）`, (await pg.getByRole("tab", { name: /公开/ }).innerText()).includes(String(pub)));
    await pg.getByRole("tab", { name: /全部/ }).click();
    await pg.fill("input[aria-label='按仓库名搜索']", "pytorch");
    await pg.waitForFunction(() => document.querySelectorAll("tbody tr[data-repo]").length === 1, null, { timeout: 10000 });
    check("仓库名搜索交给平台 keyWords", platform.imageCalls.some((c) => c.keyWords === "pytorch"));
    await pg.locator("tbody tr[data-repo='1']").click();
    await pg.waitForSelector("text=2.4-cuda12.4", { timeout: 5000 });
    check("点仓库能看到镜像版本", true);
    await pg.getByRole("button", { name: "关闭" }).click();
    await pg.fill("input[aria-label='按仓库名搜索']", "");
    await shot(pg, "11-images.png");
  });

  // ---------- 7. 取消自己的作业后释放配额 ----------
  await step("取消作业", async () => {
    await B.page.getByRole("button", { name: "作业", exact: true }).click();
    await B.page.getByRole("tab", { name: /运行中/ }).click();
    await B.page.waitForSelector(`tr[data-job='${jobId}']`, { timeout: 10000 });
    B.page.once("dialog", (d) => d.accept());
    await B.page.locator(`tr[data-job='${jobId}']`).getByRole("button", { name: "取消" }).click();
    check("取消成功提示", await waitToast(B.page, /已取消/));
    check("平台上作业已停止", platform.jobs.find((j) => j.jobId === jobId)?.status === "STOPPED");
    await B.page.reload({ waitUntil: "networkidle" });
    await B.page.waitForSelector("aside nav");
    check("取消后配额释放为 0 / 4 卡", await B.page.locator("aside").getByText("GPU 已用 0 / 4 卡").isVisible());
  });

  // ---------- 7b. 登录安全：单点登录踢下线、管理员看在线会话、换设备掉线 ----------
  await step("登录安全", async () => {
    await A.page.getByRole("button", { name: "用户与权限" }).click();
    await A.page.getByRole("tab", { name: "登录安全" }).click();
    check("管理员能看到成员登录限制设置", await A.page.getByText("成员登录限制").isVisible());
    await A.page.getByRole("tab", { name: "在线会话" }).click();
    await A.page.waitForSelector("[data-session]", { timeout: 10000 });
    check("在线会话里有 alice 和她的设备指纹", (await A.page.locator("[data-session=alice]").innerText()).includes(deviceOf(B.page)));
    check("自己的当前会话不能踢", (await A.page.locator("[data-session=boss]").getByRole("button", { name: "踢下线" }).count()) === 0);
    await A.page.getByRole("tab", { name: /用户与配额/ }).click();
    // 单个成员逐项覆盖：只把 alice 的单点登录改成「不限制」，其余跟随全局
    const card = A.page.locator("[data-user=alice]");
    await card.getByRole("button", { name: /登录限制/ }).click();
    await card.getByRole("group", { name: "单点登录来源" }).getByRole("button", { name: "单独设置" }).click();
    await card.getByLabel("不限制").check();
    await card.getByRole("button", { name: "保存登录限制" }).click();
    check("单个成员的登录限制保存成功", await waitToast(A.page, /alice 的登录限制已保存/));
    await A.page.waitForFunction(() => document.querySelector("[data-user=alice] [data-testid=effective]")?.textContent?.includes("不限制；"), null, { timeout: 10000 });
    check("实际生效里单点登录是「不限制」，绑定仍跟随全局", (await card.getByTestId("effective").innerText()).includes("绑定 浏览器、设备指纹"));
    check("卡片标出有单独设置", (await card.innerText()).includes("有单独设置"));
    await shot(A.page, "12-login-rules.png", true);
    // 恢复跟随全局，后面的踢下线测试按全局的「踢掉旧会话」执行
    await card.getByRole("button", { name: "全部恢复跟随全局" }).click();
    check("恢复跟随全局", await waitToast(A.page, /alice 的登录限制已保存/));
    await A.page.waitForFunction(() => document.querySelector("[data-user=alice]")?.textContent?.includes("全部跟随全局"), null, { timeout: 10000 });
    // alice 在另一个浏览器上下文登录，默认策略会把 B 挤下线
    const C = await open();
    await C.page.waitForSelector("#auth-user");
    await C.page.fill("#auth-user", "alice");
    await C.page.fill("#auth-pass", "alicepass1");
    await C.page.click("form button[type=submit]");
    await C.page.waitForSelector("aside nav", { timeout: 10000 });
    await B.page.reload({ waitUntil: "networkidle" });
    await B.page.waitForSelector("[data-testid=auth-notice]", { timeout: 10000 });
    check("旧页面被挤下线并说明原因", (await B.page.getByTestId("auth-notice").innerText()).includes("别处登录"));
    await shot(B.page, "13-kicked.png");
    // 伪造设备头访问：会话立即作废
    const status = await C.page.evaluate(async () => (await fetch("/api/state", { headers: { "X-QD-Device": "ffffffffffffffff" } })).status);
    check("换设备指纹的请求返回 401", status === 401, String(status));
    await C.page.reload({ waitUntil: "networkidle" });
    await C.page.waitForSelector("[data-testid=auth-notice]", { timeout: 10000 });
    check("换设备后提示登录环境变了", (await C.page.getByTestId("auth-notice").innerText()).includes("登录环境变了"));
    await C.ctx.close();
    // B 重新登录，供后面的退出步骤使用
    await B.page.fill("#auth-user", "alice");
    await B.page.fill("#auth-pass", "alicepass1");
    await B.page.click("form button[type=submit]");
    await B.page.waitForSelector("aside nav", { timeout: 10000 });
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
