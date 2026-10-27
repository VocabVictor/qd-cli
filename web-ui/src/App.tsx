import { useEffect, useState } from "react";
import { Home, Rocket, Boxes, Server, Package, Settings as Cog, LogOut, Gauge, Upload, Users } from "lucide-react";
import { api, AppState, LocalAuthState, setMe } from "./lib/api";
import { ToastHost, Spinner, clearToasts } from "./components/ui";
import { AuthGate } from "./pages/AuthGate";
import { Login } from "./pages/Login";
import { Overview } from "./pages/Overview";
import { Jobs } from "./pages/Jobs";
import { Devs } from "./pages/Devs";
import { Nodes } from "./pages/Nodes";
import { Images } from "./pages/Images";
import { SettingsPage } from "./pages/Settings";
import { Quota } from "./pages/Quota";
import { Transfer } from "./pages/Transfer";
import { Admin } from "./pages/Admin";

type NavItem = { v: string; label: string; icon: typeof Home };
const WORK: NavItem[] = [
  { v: "overview", label: "概览", icon: Home },
  { v: "jobs", label: "作业", icon: Rocket },
  { v: "devs", label: "开发环境", icon: Boxes },
];
const RES: NavItem[] = [
  { v: "nodes", label: "节点", icon: Server },
  { v: "quota", label: "配额", icon: Gauge },
  { v: "images", label: "镜像", icon: Package },
  { v: "transfer", label: "传输", icon: Upload },
];

const PAGES: Record<string, (p: { state: AppState }) => JSX.Element> = {
  overview: Overview, jobs: Jobs, devs: Devs, admin: Admin,
  nodes: Nodes, quota: Quota, transfer: Transfer, images: Images, settings: SettingsPage,
};

export function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [localAuth, setLocalAuth] = useState<LocalAuthState | null>(null);
  const [view, setView] = useState(location.hash.slice(1) || "overview");
  const [booting, setBooting] = useState(true);

  const reload = () => api.state().then(setState).catch(() => setState({ loggedIn: false })).finally(() => setBooting(false));

  // 本机账号墙（只在 --host 公开绑定时存在）要先过，再轮到平台登录。
  const bootstrap = () => {
    setBooting(true);
    api.localAuth.state()
      .then((la) => {
        setLocalAuth((prev) => { if (prev?.loggedIn !== la.loggedIn) clearToasts(); return la; }); setMe(la);
        if (la.required && !la.loggedIn) { setBooting(false); return; }
        return reload();
      })
      .catch(() => {
        const local: LocalAuthState = { required: false, hasAccount: true, loggedIn: true, role: "admin" };
        setLocalAuth(local); setMe(local); return reload();
      });
  };
  useEffect(() => { bootstrap(); }, []);
  useEffect(() => {
    const onHash = () => setView(location.hash.slice(1) || "overview");
    // 401 可能来自本机账号过期，也可能是平台 token 失效；重新走一遍启动流程分辨。
    const onUnauth = () => bootstrap();
    window.addEventListener("hashchange", onHash);
    window.addEventListener("qd-unauth", onUnauth);
    return () => { window.removeEventListener("hashchange", onHash); window.removeEventListener("qd-unauth", onUnauth); };
  }, []);

  const admin = localAuth?.role === "admin";
  const groups: [string, NavItem[]][] = [["工作台", WORK], ["资源", RES],
    ["系统", [...(admin && localAuth?.required ? [{ v: "admin", label: "用户与权限", icon: Users }] : []), { v: "settings", label: "设置", icon: Cog }]]];
  const go = (v: string) => { location.hash = v; setView(v); };
  const Page = (view === "admin" && !admin ? undefined : PAGES[view]) || Overview;
  const usedTotal = localAuth?.usage?.total ?? 0, quotaTotal = localAuth?.quota?.total ?? 0;

  const logoutAll = async () => {
    if (localAuth?.required) { await api.localAuth.logout().catch(() => {}); bootstrap(); }
    else { await api.logout().catch(() => {}); reload(); }
  };

  return (
    <ToastHost>
      {booting ? (
        <div className="h-full grid place-items-center"><Spinner label="连接中…" /></div>
      ) : localAuth?.required && !localAuth.loggedIn ? (
        <AuthGate hasAccount={localAuth.hasAccount} notice={localAuth.notice} onAuthed={bootstrap} />
      ) : !state?.loggedIn ? (
        admin ? <Login onLogin={reload} /> : (
          <div className="h-full grid place-items-center px-4">
            <div className="card p-7 max-w-sm text-center animate-pop">
              <div className="text-[15px] font-semibold mb-1">平台账号未登录</div>
              <p className="text-body text-ink-soft mb-4">这台机器上的 GPU 平台登录已失效，请联系管理员重新登录。</p>
              <button className="btn" onClick={logoutAll}><LogOut size={13} />退出</button>
            </div>
          </div>
        )
      ) : (
        <div className="flex h-full">
          <aside className="w-[232px] shrink-0 flex flex-col bg-side text-white">
            <div className="flex items-center gap-2.5 h-16 px-5">
              <span className="grid place-items-center w-8 h-8 rounded-[9px] bg-gradient-to-br from-[#818CF8] to-[#4F46E5] text-white text-[13px] font-bold tracking-tight shadow-[0_4px_12px_-2px_rgba(99,102,241,.6)]">qd</span>
              <div className="min-w-0">
                <div className="text-[14px] font-semibold leading-tight tracking-tight">qd 控制台</div>
                <div className="text-[11px] text-side-text leading-tight mt-0.5">GPU 集群工作台</div>
              </div>
            </div>
            <nav className="px-3 flex-1 overflow-y-auto pb-3" aria-label="主导航">
              {groups.map(([title, items]) => (
                <div key={title} className="mt-4 first:mt-1">
                  <div className="px-2.5 mb-1 text-[11px] font-medium text-[#5B6478] tracking-wide">{title}</div>
                  <div className="space-y-0.5">
                    {items.map(({ v, label, icon: Icon }) => (
                      <button key={v} onClick={() => go(v)} className="nav-item" aria-current={view === v ? "page" : undefined}>
                        <Icon size={16} strokeWidth={1.8} className={view === v ? "text-[#A5B4FC]" : "text-[#64748B]"} />{label}
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </nav>
            <div className="m-3 p-3 rounded-[10px] bg-side-soft border border-side-line">
              <div className="flex items-center gap-2.5">
                <span className="grid place-items-center w-8 h-8 rounded-full bg-gradient-to-br from-[#334155] to-[#1E293B] text-[12px] font-semibold text-white uppercase shrink-0 ring-1 ring-white/10">
                  {(localAuth?.username || state.username || "?").slice(0, 1)}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="text-[13px] font-medium truncate">{localAuth?.required ? localAuth.username : state.username}</div>
                  <div className="text-[11px] text-side-text truncate">
                    {!localAuth?.required ? "本机模式" : admin ? "管理员" : "成员"}
                  </div>
                </div>
                <button className="grid place-items-center w-7 h-7 rounded-ctl text-side-text hover:text-white hover:bg-white/10 transition-colors" title="退出登录" aria-label="退出" onClick={logoutAll}><LogOut size={15} /></button>
              </div>
              {localAuth?.required && !admin && (
                <div className="mt-3">
                  <div className="text-[11px] text-side-text mb-1.5 tabular-nums">GPU 已用 {usedTotal} / {quotaTotal} 卡</div>
                  <div className="h-1 rounded-full bg-white/10 overflow-hidden"><i className="block h-full rounded-full bg-[#818CF8]" style={{ width: `${quotaTotal ? Math.min(100, (usedTotal / quotaTotal) * 100) : 0}%` }} /></div>
                </div>
              )}
            </div>
          </aside>
          <main className="flex-1 overflow-y-auto">
            <div key={view} className="max-w-[1440px] mx-auto px-8 py-7 animate-rise"><Page state={state} /></div>
          </main>
        </div>
      )}
    </ToastHost>
  );
}
