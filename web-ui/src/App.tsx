import { useEffect, useState } from "react";
import { Home, Rocket, Boxes, Server, Package, Settings as Cog, LogOut, Gauge, Upload, Users } from "lucide-react";
import { api, AppState, LocalAuthState, setMe } from "./lib/api";
import { ToastHost, Spinner } from "./components/ui";
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

const NAV = [
  { v: "overview", label: "概览", icon: Home },
  { v: "jobs", label: "作业", icon: Rocket },
  { v: "devs", label: "开发环境", icon: Boxes },
  { v: "nodes", label: "节点", icon: Server },
  { v: "quota", label: "配额", icon: Gauge },
  { v: "transfer", label: "传输", icon: Upload },
  { v: "images", label: "镜像", icon: Package },
  { v: "settings", label: "设置", icon: Cog },
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
        setLocalAuth(la); setMe(la);
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
  const nav = admin && localAuth?.required ? [...NAV.slice(0, -1), { v: "admin", label: "用户与配额", icon: Users }, NAV[NAV.length - 1]] : NAV;
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
        <AuthGate hasAccount={localAuth.hasAccount} onAuthed={bootstrap} />
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
          <aside className="w-[220px] shrink-0 border-r border-line flex flex-col bg-canvas">
            <div className="flex items-center gap-2.5 h-14 px-4">
              <span className="grid place-items-center w-6 h-6 rounded-[5px] bg-ink text-white text-[11px] font-semibold">qd</span>
              <div className="min-w-0">
                <div className="text-[13px] font-semibold leading-tight">qd 控制台</div>
                <div className="text-[11px] text-ink-faint leading-tight">v{state.version}</div>
              </div>
            </div>
            <nav className="px-2 flex-1 space-y-px overflow-y-auto" aria-label="主导航">
              {nav.map(({ v, label, icon: Icon }) => (
                <button key={v} onClick={() => go(v)} className="nav-item" aria-current={view === v ? "page" : undefined}>
                  <Icon size={15} strokeWidth={1.8} className={view === v ? "text-ink" : "text-ink-faint"} />{label}
                </button>
              ))}
            </nav>
            <div className="border-t border-line px-3 py-3 flex items-center gap-2.5">
              <span className="grid place-items-center w-7 h-7 rounded-full bg-selected text-[12px] font-semibold text-ink-soft uppercase shrink-0">
                {(localAuth?.username || state.username || "?").slice(0, 1)}
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-medium truncate flex items-center gap-1.5">
                  <span className="truncate">{localAuth?.required ? localAuth.username : state.username}</span>
                  {localAuth?.required && admin && <span className="tag tag-violet">管理员</span>}
                </div>
                <div className="text-[11px] text-ink-faint truncate">
                  {localAuth?.required && !admin ? `GPU 已用 ${usedTotal} / ${quotaTotal} 卡` : `平台账号 ${state.username}`}
                </div>
              </div>
              <button className="btn btn-ghost btn-mini px-1.5 text-ink-faint" title="退出" aria-label="退出" onClick={logoutAll}><LogOut size={14} /></button>
            </div>
          </aside>
          <main className="flex-1 overflow-y-auto">
            <div className="max-w-[1480px] mx-auto px-6 py-5"><Page state={state} /></div>
          </main>
        </div>
      )}
    </ToastHost>
  );
}
