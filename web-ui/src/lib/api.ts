export interface AppState {
  loggedIn: boolean;
  username?: string;
  spaceId?: string;
  spaces?: { id: string; name: string }[];
  baseUrl?: string;
  version?: string;
  concurrency?: number;
  insecureTls?: boolean;
}

/** GPU 卡数上限 / 占用：总数、按空间 ID、按资源组 ID。 */
export interface GpuLimits { total: number; spaces: Record<string, number>; groups: Record<string, number> }

/** 本机 qd web 账号（谁能打开这台机器上的控制面板），和上面的平台账号无关。 */
export interface LocalAuthState {
  required: boolean;
  hasAccount: boolean;
  loggedIn: boolean;
  username?: string;
  role?: "admin" | "user";
  quota?: GpuLimits;
  usage?: GpuLimits;
}

export interface AdminUser { username: string; role: "admin" | "user"; quota: GpuLimits; usage: GpuLimits; createdAt: number }

/** 当前本机用户，App 启动时写入，页面据此隐藏无权限的按钮（服务端仍会再校验）。 */
let me: LocalAuthState = { required: false, hasAccount: true, loggedIn: true, role: "admin" };
export const setMe = (s: LocalAuthState) => { me = s; };
export const getMe = () => me;
export const isAdmin = () => me.role === "admin";
/** 作业是否可由当前用户操作：管理员全部可以，普通用户只能操作 qd web 记录在自己名下的。 */
export const canManageJob = (job: any) => isAdmin() || (!!me.username && String(job?.qdOwner || "").toLowerCase() === me.username.toLowerCase());

async function qfetch<T = any>(url: string, opts?: RequestInit): Promise<T> {
  const r = await fetch(url, opts);
  let j: any = null;
  try { j = await r.json(); } catch { throw new Error("响应不是 JSON (HTTP " + r.status + ")"); }
  // 登录/注册接口的 401 是"密码或邀请码不对"，交给表单自己提示；其他接口的 401 才表示会话失效。
  if (r.status === 401 && !url.startsWith("/api/auth/")) window.dispatchEvent(new CustomEvent("qd-unauth"));
  if (r.status === 401) throw new Error(j?.error || "未登录");
  if (!j.ok) throw new Error(j.error || "HTTP " + r.status);
  return j.data as T;
}

export const qget = <T = any>(url: string) => qfetch<T>(url);
export const qpost = <T = any>(url: string, body?: any) =>
  qfetch<T>(url, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body || {}) });

export const pcall = <T = any>(method: string, service: string, path: string, data?: any, spaceId?: string) =>
  qpost<T>("/api/call", { method, service, path, data: data || {}, ...(spaceId ? { spaceId } : {}) });

export const api = {
  state: () => qget<AppState>("/api/state"),
  login: (username: string, password: string, ldap: boolean, remember: boolean) =>
    qpost("/api/login", { username, password, ldap, remember }),
  logout: () => qpost("/api/logout"),
  idle: () => qget("/api/idle"),
  /** 节点（集群级）走平台原生分页：group 资源组 ID，model 完整 GPU 型号，minGpu 至少空闲几卡，sort gpu/cpu/memory。 */
  nodes: (o: { page?: number; size?: number; group?: string; model?: string; minGpu?: number; sort?: string; dir?: string } = {}) =>
    qget(`/api/nodes?${toQuery({ page: o.page || 1, size: o.size || 50, ...o })}`),
  nodeFacets: () => qget<{ models: string[]; groups: string[] }>("/api/nodes/facets"),
  /** 镜像仓库跨空间走平台原生分页：source 为 current/public/shared/mine，q 为仓库名关键字。 */
  images: (o: { page?: number; size?: number; source?: string; q?: string; space?: string } = {}) =>
    qget(`/api/images?${toQuery({ page: o.page || 1, size: o.size || 50, ...o })}`),
  /** 作业列表走平台原生分页：status 为逗号分隔的平台状态，q 为作业名关键字。 */
  jobs: (o: { scope?: string; page?: number; size?: number; status?: string; q?: string; space?: string } = {}) => {
    const p = new URLSearchParams({ scope: o.scope || "mine", page: String(o.page || 1), size: String(o.size || 50) });
    if (o.status) p.set("status", o.status);
    if (o.q) p.set("q", o.q);
    if (o.space) p.set("space", o.space);
    return qget(`/api/jobs?${p}`);
  },
  job: (id: string, sp: string) => qget(`/api/job/${id}${sp ? "?spaceId=" + sp : ""}`),
  jobLogs: (id: string, limit: number, sp: string) => qget(`/api/job/${id}/logs?limit=${limit}${sp ? "&spaceId=" + sp : ""}`),
  jobMetrics: (id: string, minutes: number, sp: string) => qget(`/api/job/${id}/metrics?minutes=${minutes}${sp ? "&spaceId=" + sp : ""}`),
  bulk: (action: string, ids: string[], spaceId: string) => qpost("/api/bulk", { action, ids, spaceId: spaceId || "" }),
  devs: (scope: string) => qget("/api/devs?scope=" + scope),
  config: (key: string, value: string) => qpost("/api/config", { key, value }),
  keepalive: (action: string) => qpost("/api/keepalive", { action, minutes: 30 }),
  exec: (body: any) => qpost("/api/exec", body),
  templates: () => qget("/api/templates"),
  saveTemplate: (name: string, kind: "job" | "dev", payload: any) => qpost("/api/templates", { name, kind, payload }),
  deleteTemplate: (name: string) => qfetch("/api/templates/" + encodeURIComponent(name), { method: "DELETE" }),
  localAuth: {
    state: () => qget<LocalAuthState>("/api/auth/state"),
    login: (username: string, password: string) => qpost("/api/auth/login", { username, password }),
    register: (username: string, password: string, inviteCode: string) =>
      qpost("/api/auth/register", { username, password, inviteCode }),
    logout: () => qpost("/api/auth/logout"),
    changePassword: (oldPassword: string, newPassword: string) => qpost("/api/auth/password", { oldPassword, newPassword }),
  },
  admin: {
    users: () => qget<{ users: AdminUser[] }>("/api/admin/users"),
    setQuota: (name: string, quota: GpuLimits) => qpost(`/api/admin/users/${encodeURIComponent(name)}/quota`, quota),
    setRole: (name: string, role: "admin" | "user") => qpost(`/api/admin/users/${encodeURIComponent(name)}/role`, { role }),
    resetPassword: (name: string, password: string) => qpost(`/api/admin/users/${encodeURIComponent(name)}/password`, { password }),
    remove: (name: string) => qfetch(`/api/admin/users/${encodeURIComponent(name)}`, { method: "DELETE" }),
  },
};

// helpers
export const fmtMiB = (m?: number) => {
  const n = Number(m) || 0;
  if (n >= 1024 * 1024) return (n / 1024 / 1024).toFixed(2) + " TiB";
  if (n >= 1024) return (n / 1024).toFixed(1) + " GiB";
  return n + " MiB";
};
export const fmtCpu = (c?: number) => {
  const n = Number(c) || 0;
  if (n >= 1000) return (n / 1000).toFixed(1) + "k";
  // 平台的 cpuRequest 是浮点累加值，直接 String() 会漏出 116.68499999999996 这种尾数
  return String(Math.round(n * 100) / 100);
};
export const fmtTime = (t?: any) => {
  if (!t) return "-";
  const d = new Date(typeof t === "number" && t < 1e12 ? t * 1000 : t);
  return isNaN(d.getTime()) ? String(t) : d.toLocaleString("zh-CN", { hour12: false });
};
/** 平台的 gpuModels / resourceGroups 是逗号分隔的字符串，也兼容数组。 */
export const splitList = (v: any): string[] =>
  Array.isArray(v) ? v.map(String) : typeof v === "string" ? v.split(",").map((s) => s.trim()).filter(Boolean) : [];

/** 拼查询串，空值不带。 */
const toQuery = (o: Record<string, any>) => {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(o)) if (v !== undefined && v !== null && v !== "" && v !== 0) p.set(k, String(v));
  return p.toString();
};

export const asList = (v: any): any[] => Array.isArray(v) ? v : (v === undefined || v === null || v === "" ? [] : [v]);
export const field = (o: any, keys: string[], dflt: any = "-") => {
  for (const k of keys) { const v = o?.[k]; if (v !== undefined && v !== null && v !== "") return v; }
  return dflt;
};

export const spacesOf = (s?: AppState) =>
  s?.spaces?.length ? s.spaces : [{ id: s?.spaceId || "", name: "默认" }];

type SpaceResult<T> = { space: { id: string; name: string }; data: T };
export async function eachSpace<T>(s: AppState, fn: (sp: { id: string; name: string }) => Promise<T>): Promise<SpaceResult<T>[]> {
  const results = await Promise.all(
    spacesOf(s).map((sp) => fn(sp).then((data): SpaceResult<T> => ({ space: sp, data })).catch(() => null))
  );
  return results.filter((x): x is SpaceResult<T> => x !== null);
}

/** 平台返回的终端地址常指向公网 IP（浏览器走系统代理时连不上）。
 *  把主机换成 baseUrl 的主机（内网地址），端口/路径/查询全部保留。 */
export function localizeTerminalUrl(url: string, baseUrl?: string): string {
  try {
    if (!url || !baseUrl) return url;
    const u = new URL(url), b = new URL(baseUrl);
    if (u.hostname === b.hostname) return url;
    u.hostname = b.hostname;
    return u.toString();
  } catch { return url; }
}
