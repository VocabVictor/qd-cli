import { useEffect, useState } from "react";
import { clsx } from "clsx";
import { Home, Rocket, Boxes, Server, Package, Settings as Cog, LogOut, Zap, Gauge, Upload } from "lucide-react";
import { api, AppState } from "./lib/api";
import { ToastHost, Spinner } from "./components/ui";
import { Login } from "./pages/Login";
import { Overview } from "./pages/Overview";
import { Jobs } from "./pages/Jobs";
import { Devs } from "./pages/Devs";
import { Nodes } from "./pages/Nodes";
import { Images } from "./pages/Images";
import { SettingsPage } from "./pages/Settings";
import { Quota } from "./pages/Quota";
import { Transfer } from "./pages/Transfer";

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
  overview: Overview, jobs: Jobs, devs: Devs,
  nodes: Nodes, quota: Quota, transfer: Transfer, images: Images, settings: SettingsPage,
};

export function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [view, setView] = useState(location.hash.slice(1) || "overview");
  const [booting, setBooting] = useState(true);

  const reload = () => api.state().then(setState).catch(() => setState({ loggedIn: false })).finally(() => setBooting(false));
  useEffect(() => { reload(); }, []);
  useEffect(() => {
    const onHash = () => setView(location.hash.slice(1) || "overview");
    const onUnauth = () => setState((s) => (s ? { ...s, loggedIn: false } : s));
    window.addEventListener("hashchange", onHash);
    window.addEventListener("qd-unauth", onUnauth);
    return () => { window.removeEventListener("hashchange", onHash); window.removeEventListener("qd-unauth", onUnauth); };
  }, []);

  const go = (v: string) => { location.hash = v; setView(v); };
  const Page = PAGES[view] || Overview;

  return (
    <ToastHost>
      <div className="h-full w-1 fixed top-0 left-0 right-0 z-10" style={{ height: 3, width: "100%", background: "#1070FE" }} />
      {booting ? (
        <div className="h-full grid place-items-center"><Spinner label="连接中…" /></div>
      ) : !state?.loggedIn ? (
        <Login onLogin={reload} />
      ) : (
        <div className="flex h-full pt-1">
          <aside className="w-56 shrink-0 border-r border-line bg-surface flex flex-col">
            <div className="px-5 pt-5 pb-4">
              <div className="flex items-center gap-2">
                <span className="grid place-items-center w-8 h-8 rounded-xl text-white" style={{ background: "#1070FE" }}><Zap size={17} /></span>
                <div>
                  <div className="font-semibold leading-tight">qd</div>
                  <div className="text-[11px] text-ink-faint leading-tight">GPU 平台 · v{state.version}</div>
                </div>
              </div>
            </div>
            <nav className="px-3 flex-1 space-y-0.5">
              {NAV.map(({ v, label, icon: Icon }) => (
                <button key={v} onClick={() => go(v)}
                  className={clsx("w-full flex items-center gap-2.5 px-3 h-10 rounded-ctl text-[13px] transition-colors",
                    view === v ? "bg-brand text-white font-medium" : "text-ink-soft hover:bg-canvas")}>
                  <Icon size={17} className={view === v ? "text-white" : "text-ink-faint"} />{label}
                </button>
              ))}
            </nav>
            <div className="p-3 border-t border-line">
              <div className="flex items-center justify-between px-2">
                <div className="min-w-0">
                  <div className="text-sm font-medium truncate">{state.username}</div>
                  <div className="text-[11px] text-ink-faint truncate">空间 {state.spaceId}</div>
                </div>
                <button className="btn btn-mini" onClick={() => api.logout().then(reload)}><LogOut size={13} />退出</button>
              </div>
            </div>
          </aside>
          <main className="flex-1 overflow-y-auto"><div className="max-w-[1560px] mx-auto px-6 py-5"><Page state={state} /></div></main>
        </div>
      )}
    </ToastHost>
  );
}
