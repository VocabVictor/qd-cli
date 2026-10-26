// 本地模拟 GPU 平台：实现 qd 后端会调用的接口（空间、资源组、节点、项目、作业、镜像、监控、日志、
// 网页终端页面与 WebSocket），让浏览器测试能完整走提交 / 监控 / 终端流程，不碰真实集群。
import http from "node:http";
import crypto from "node:crypto";

const CORE = "/gemini/v1/gemini_api/gemini_api";
const TOOL = "/gemini/v1/geminitool_api/geminitool_api";

export function createPlatform() {
  const spaces = [
    { spaceId: "s1", spaceName: "训练空间" },
    { spaceId: "s2", spaceName: "推理空间" },
  ];
  const groups = [
    { id: "g1", space: "s1", name: "A100 训练池", card: "A100-80G", per: 8, free: [8, 6, 3, 0] },
    { id: "g2", space: "s1", name: "H800 大模型池", card: "H800", per: 8, free: [2, 0] },
    { id: "g3", space: "s2", name: "L40 推理池", card: "L40S", per: 4, free: [4, 4, 3, 2, 1, 0] },
    { id: "g4", space: "s2", name: "CPU 通用池", card: "", per: 0, free: [] },
  ];
  const nodes = groups.flatMap((g) => g.free.map((free, i) => ({
    name: `${g.id}-node-${String(i + 1).padStart(2, "0")}`, ip: `10.0.${g.id.slice(1)}.${i + 1}`,
    gpuLeft: free, gpuTotal: g.per, gpuModels: [g.card], resourceGroups: [g.name], space: g.space, group: g.id,
  })));
  const projects = [
    { projectId: "p1", projectName: "训练空间", space: "s1", jobenvId: 501 },
    { projectId: "p2", projectName: "推理空间", space: "s2", jobenvId: 0 },
  ];
  const now = Date.now();
  const jobs = [
    { jobId: "880001", jobName: "llama-sft-baseline", status: "RUNNING", statGpu: 8, createTime: now - 3 * 3600e3, projectId: "p1", space: "s1", rsgroupId: "g1", gpu: 8, gpuType: "A100-80G" },
    { jobId: "880002", jobName: "eval-mmlu", status: "SUCCEEDED", statGpu: 1, createTime: now - 26 * 3600e3, projectId: "p1", space: "s1", rsgroupId: "g1", gpu: 1, gpuType: "A100-80G" },
  ];
  let seq = 900000;
  let port = 0;
  const submitted = [];

  const ok = (data) => ({ code: 0, msg: "成功", data });
  const spaceOf = (req) => String(req.headers["spaceid"] || "s1");
  const groupOf = (id) => groups.find((g) => g.id === id);
  const terminalUrl = (id) => `http://127.0.0.1:${port}/terminal?namespace=ns-${id}&pod=pod-${id}&container=main&token=tok${id}`;
  const series = (seed, n = 40) => Array.from({ length: n }, (_, i) => ({ value: String((50 + 35 * Math.sin((i + seed) / 5)).toFixed(1)) }));

  function core(method, path, query, body, req) {
    const sp = spaceOf(req);
    let m;
    if (path === "/space/list") return ok({ userSpaceList: spaces });
    if (path === "/space/rsgroup/bindlist") return ok({ rsgroupList: groups.filter((g) => g.space === sp).map((g) => ({ rsgroupId: g.id })) });
    if ((m = path.match(/^\/rsgroup\/detail\/(\w+)$/))) {
      const g = groupOf(m[1]);
      return ok({ rsgroupName: g.name, graphicsCards: g.card, nodeCount: Math.max(1, g.free.length), cpuTotal: 256000, memoryTotal: 1048576, gpuTotal: g.per * g.free.length });
    }
    if (path === "/job/jobResourceStock") {
      const g = groupOf(body.rsgroupId);
      return ok({ cpu: 96, memory: 393216, gpuCount: g && g.free.length ? Math.max(...g.free) : 0 });
    }
    if (path === "/jobQueue/node/job/list") {
      const list = nodes.filter((n) => (query.rsgroup ? n.group === query.rsgroup : n.space === sp));
      return ok({ nodes: list, total: list.length });
    }
    if (path === "/project/list") return ok({ projectList: projects.filter((p) => p.space === sp), listCount: 1 });
    if ((m = path.match(/^\/project\/job\/list\/(\w+)$/))) return ok({ jobList: jobs.filter((j) => j.projectId === m[1]) });
    if (path === "/job/new" && method === "POST") {
      const roles = body.taskroles || [];
      const gpu = roles.reduce((a, r) => a + (Number(r.gpu) || 0) * (Number(r.instance) || 1), 0);
      const job = { jobId: String(++seq), jobName: body.jobName, status: "RUNNING", statGpu: gpu, createTime: Date.now(), projectId: body.projectId,
        space: body.spaceId, rsgroupId: body.rsgroupId, gpu, gpuType: roles[0]?.gpuType || "", runScript: roles[0]?.runScript, imageId: body.imageId };
      jobs.unshift(job);
      submitted.push(body);
      return ok({ jobId: job.jobId });
    }
    if ((m = path.match(/^\/job\/detail\/(\w+)$/))) {
      const j = jobs.find((x) => x.jobId === m[1]);
      if (!j) return { code: 404001, msg: "作业不存在" };
      return ok({ ...j, spaceId: j.space, imageId: j.imageId || 11, maxRunHour: 24, startTime: j.createTime,
        taskroleList: [{ runScript: j.runScript || "python train.py", cpu: 8, memory: 32768, storage: 102400, gpu: j.gpu, gpuType: j.gpuType,
          instanceList: [{ instanceId: Number(j.jobId), status: j.status, terminalUrl: j.status === "RUNNING" ? terminalUrl(j.jobId) : "-" }] }] });
    }
    if ((m = path.match(/^\/job\/cancel\/(\w+)$/))) { const j = jobs.find((x) => x.jobId === m[1]); if (j) j.status = "STOPPED"; return ok({}); }
    if ((m = path.match(/^\/job\/delete\/(\w+)$/))) { const i = jobs.findIndex((x) => x.jobId === m[1]); if (i >= 0) jobs.splice(i, 1); return ok({}); }
    if (path === "/imageRepository/list") return ok({ imageList: [{ imageRepositoryId: "r1", originalName: "pytorch" }, { imageRepositoryId: "r2", originalName: "vllm" }] });
    if ((m = path.match(/^\/imageRepository\/detail\/(\w+)$/))) return ok({ imageList: m[1] === "r1" ? [{ imageId: 11, imageTag: "2.4-cuda12.4" }, { imageId: 12, imageTag: "2.3-cuda12.1" }] : [{ imageId: 21, imageTag: "0.6.3" }] });
    if (path === "/quota/available") return ok({ gpuCardCnt: 64, usedGpuCardCnt: 9, leftGpuCardCnt: 55, gpuquotaDataList: [{ cardCnt: 64 }], cpu: 1024, usedCpu: 80, leftCpu: 944, memory: 4194304, usedMemory: 327680, leftMemory: 3866624, storage: 10240000, usedStorage: 204800, leftStorage: 10035200 });
    if ((m = path.match(/^\/jobenv\/refresh\/(\w+)$/))) return ok({ status: "RUNNING", devJobDetail: { taskroleList: [{ instanceList: [{ terminalUrl: terminalUrl("dev" + m[1]) }] }] } });
    if (path === "/sftp/channel/list") return ok({ sftpList: [], listCount: 0 });
    return null;
  }

  function tool(path, query) {
    let m;
    if ((m = path.match(/^\/user\/job\/chart\/(\w+)$/))) {
      const seed = { gpu_util: 0, gpu_memory: 7, cpu: 13, memory: 21 }[m[1]] ?? 0;
      return { option: { series: [{ data: series(seed) }] } };
    }
    if (path === "/user/job/log/newList") {
      const logList = Array.from({ length: 30 }, (_, i) => ({ message: `[rank0] epoch 1 step ${(i + 1) * 10} loss ${(2.4 - i * 0.05).toFixed(3)}`, timestamp: (Date.now() - 60e3 + i * 1000) * 1e6 }));
      return ok({ logList });
    }
    return null;
  }

  const TERMINAL_HTML = `<!doctype html><html><head><meta charset="utf-8"><title>terminal</title>
<style>body{margin:0;background:#1c1c1e;color:#e5e5e5;font:13px/1.5 Menlo,monospace}#out{white-space:pre-wrap;padding:10px;min-height:200px}#in{width:100%;box-sizing:border-box;background:#111;color:#fff;border:0;padding:8px;font:inherit}</style>
</head><body><div id="out"></div><input id="in" aria-label="terminal-input" autofocus><script src="/static/terminal.js"></script></body></html>`;
  const TERMINAL_JS = `(function(){var p=new URLSearchParams(location.search);var scheme="wss://";
var url=scheme+location.host+"/ws/"+p.get("namespace")+"/"+p.get("pod")+"/"+p.get("container")+"/"+p.get("token")+"/webshell";
var out=document.getElementById("out"),inp=document.getElementById("in");var ws=new WebSocket(url);
ws.onopen=function(){out.textContent+="connected "+p.get("pod")+"\\n$ "};ws.onmessage=function(e){out.textContent+=e.data+"\\n$ "};
ws.onclose=function(){out.textContent+="\\n[closed]"};inp.addEventListener("keydown",function(e){if(e.key==="Enter"){ws.send(inp.value);out.textContent+=inp.value+"\\n";inp.value=""}})})();`;

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, "http://x");
    const query = Object.fromEntries(url.searchParams);
    let raw = "";
    req.on("data", (c) => (raw += c));
    req.on("end", () => {
      let body = {};
      try { body = raw ? JSON.parse(raw) : {}; } catch { /* 非 JSON 请求体 */ }
      const send = (status, value, type = "application/json") => { res.writeHead(status, { "content-type": type }); res.end(typeof value === "string" ? value : JSON.stringify(value)); };
      if (url.pathname === "/terminal") return send(200, TERMINAL_HTML, "text/html; charset=utf-8");
      if (url.pathname === "/static/terminal.js") return send(200, TERMINAL_JS, "application/javascript");
      let out = null;
      if (url.pathname.startsWith(CORE)) {
        const path = url.pathname.slice(CORE.length).replace(/^\/user(?=\/)/, "");
        out = core(req.method, path, query, body, req);
      } else if (url.pathname.startsWith(TOOL)) {
        out = tool(url.pathname.slice(TOOL.length), query);
      }
      if (!out) return send(404, { code: 404, msg: "mock: 未实现 " + req.method + " " + url.pathname });
      send(200, out);
    });
  });


  // 模拟容器里的 shell：按 pod 记工作目录，回答几条常用命令，其余报 command not found
  const cwd = new Map();
  function shell(line, wsPath) {
    const pod = (wsPath.split("/")[3] || "pod").replace(/^pod-/, "");
    const dir = cwd.get(pod) || "/gemini/code";
    const [cmd, ...args] = line.split(/\s+/);
    switch (cmd) {
      case "": return "";
      case "pwd": return dir;
      case "hostname": return `pod-${pod}`;
      case "whoami": return "root";
      case "echo": return args.join(" ");
      case "cd": { const to = args[0] || "/root"; cwd.set(pod, to.startsWith("/") ? to : `${dir}/${to}`); return ""; }
      case "ls": return dir === "/gemini/code" ? "main.py  configs  data  requirements.txt" : "";
      case "nvidia-smi": return "NVIDIA-SMI 550.54  Driver 550.54  CUDA 12.4\nGPU 0: A100-SXM4-80GB  45C  P0  312W / 400W  61234MiB / 81920MiB  87%";
      case "python": return args.includes("--version") ? "Python 3.11.9" : "python: missing script";
      default: return `bash: ${cmd}: command not found`;
    }
  }

  // 最小 WebSocket：完成握手，回显文本帧，供终端代理测试。
  server.on("upgrade", (req, socket) => {
    const key = req.headers["sec-websocket-key"];
    const accept = crypto.createHash("sha1").update(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest("base64");
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    const sendText = (text) => {
      const payload = Buffer.from(text);
      const head = payload.length < 126 ? Buffer.from([0x81, payload.length]) : Buffer.from([0x81, 126, payload.length >> 8, payload.length & 255]);
      socket.write(Buffer.concat([head, payload]));
    };
    let buf = Buffer.alloc(0);
    socket.on("data", (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      while (buf.length >= 2) {
        const opcode = buf[0] & 15, masked = buf[1] & 128;
        let len = buf[1] & 127, off = 2;
        if (len === 126) { if (buf.length < 4) return; len = buf.readUInt16BE(2); off = 4; }
        else if (len === 127) { if (buf.length < 10) return; len = Number(buf.readBigUInt64BE(2)); off = 10; }
        const maskOff = off; if (masked) off += 4;
        if (buf.length < off + len) return;
        const data = Buffer.from(buf.subarray(off, off + len));
        if (masked) for (let i = 0; i < data.length; i++) data[i] ^= buf[maskOff + (i % 4)];
        buf = buf.subarray(off + len);
        if (opcode === 8) { socket.end(Buffer.from([0x88, 0])); return; }
        if (opcode === 9) { socket.write(Buffer.concat([Buffer.from([0x8a, data.length]), data])); continue; }
        if (opcode === 1) sendText(shell(data.toString().trim(), req.url));
      }
    });
    socket.on("error", () => {});
  });

  return {
    jobs, submitted, nodes,
    listen: (p) => new Promise((resolve) => server.listen(p, "127.0.0.1", () => { port = server.address().port; resolve(`http://127.0.0.1:${port}`); })),
    close: () => new Promise((resolve) => { server.closeAllConnections?.(); server.close(() => resolve()); }),
  };
}

// 单独运行：node e2e/mock-platform.mjs [port]，方便手工预览界面
if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith("mock-platform.mjs")) {
  const platform = createPlatform();
  platform.listen(Number(process.argv[2] || 28180)).then((url) => console.log("mock platform:", url));
}
