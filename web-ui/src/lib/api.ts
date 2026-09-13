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

async function qfetch<T = any>(url: string, opts?: RequestInit): Promise<T> {
  const r = await fetch(url, opts);
  let j: any = null;
  try { j = await r.json(); } catch { throw new Error("响应不是 JSON (HTTP " + r.status + ")"); }
  if (r.status === 401) { window.dispatchEvent(new CustomEvent("qd-unauth")); throw new Error(j?.error || "未登录"); }
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
  jobs: (scope: string, size = 500) => qget(`/api/jobs?scope=${scope}&size=${size}`),
  job: (id: string, sp: string) => qget(`/api/job/${id}${sp ? "?spaceId=" + sp : ""}`),
  jobLogs: (id: string, limit: number, sp: string) => qget(`/api/job/${id}/logs?limit=${limit}${sp ? "&spaceId=" + sp : ""}`),
  jobMetrics: (id: string, minutes: number, sp: string) => qget(`/api/job/${id}/metrics?minutes=${minutes}${sp ? "&spaceId=" + sp : ""}`),
  bulk: (action: string, ids: string[], spaceId: string) => qpost("/api/bulk", { action, ids, spaceId: spaceId || "" }),
  devs: (scope: string) => qget("/api/devs?scope=" + scope),
  config: (key: string, value: string) => qpost("/api/config", { key, value }),
  keepalive: (action: string) => qpost("/api/keepalive", { action, minutes: 30 }),
  exec: (body: any) => qpost("/api/exec", body),
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
  return n >= 1000 ? (n / 1000).toFixed(1) + "k" : String(n);
};
export const fmtTime = (t?: any) => {
  if (!t) return "-";
  const d = new Date(typeof t === "number" && t < 1e12 ? t * 1000 : t);
  return isNaN(d.getTime()) ? String(t) : d.toLocaleString("zh-CN", { hour12: false });
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
