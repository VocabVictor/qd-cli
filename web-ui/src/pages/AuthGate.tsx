import { useState } from "react";
import { Lock, AlertTriangle } from "lucide-react";
import { api } from "../lib/api";
import { deviceId } from "../lib/device";
import { useToast } from "../components/ui";

/** 本机账号墙：只在 `qd web --host` 绑定非 127.0.0.1 时出现。邀请码决定角色
 *  （管理员码 → 管理员，普通码 → 成员，配额 0），由服务端识别，页面上只有一个输入框。 */
export function AuthGate({ hasAccount, notice, onAuthed }: { hasAccount: boolean; notice?: string; onAuthed: () => void }) {
  const toast = useToast();
  const [tab, setTab] = useState<"login" | "register">(hasAccount ? "login" : "register");
  const [u, setU] = useState("");
  const [p, setP] = useState("");
  const [invite, setInvite] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!u || !p) return toast("请输入用户名和密码", true);
    if (tab === "register" && !invite) return toast("请输入邀请码", true);
    setBusy(true);
    try {
      if (tab === "register") await api.localAuth.register(u, p, invite);
      else await api.localAuth.login(u, p);
      onAuthed();
    } catch (err: any) {
      toast(err.message || "操作失败", true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="h-full grid lg:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)] bg-white">
      <div className="hidden lg:flex flex-col justify-between bg-side text-white p-10 relative overflow-hidden">
        <div className="absolute -right-24 -top-24 w-96 h-96 rounded-full bg-[#4F46E5]/20 blur-3xl" />
        <div className="absolute -left-16 bottom-10 w-72 h-72 rounded-full bg-[#0F766E]/20 blur-3xl" />
        <div className="relative flex items-center gap-2.5">
          <span className="grid place-items-center w-9 h-9 rounded-[10px] bg-gradient-to-br from-[#818CF8] to-[#4F46E5] text-[14px] font-bold">qd</span>
          <span className="text-[15px] font-semibold">qd 控制台</span>
        </div>
        <div className="relative">
          <div className="text-[28px] leading-9 font-semibold tracking-tight">GPU 集群工作台</div>
          <p className="text-[14px] text-side-text mt-3 max-w-sm leading-6">提交作业、查看空闲卡、进终端看日志，一个页面搞定。</p>
          <div className="grid grid-cols-3 gap-3 mt-8 max-w-md">
            {[["作业", "提交与监控"], ["节点", "空闲卡一目了然"], ["终端", "浏览器里直接进"]].map(([t, d]) => (
              <div key={t} className="rounded-[10px] bg-white/[0.04] border border-white/[0.06] px-3 py-3">
                <div className="text-[13px] font-medium">{t}</div><div className="text-[11px] text-side-text mt-0.5">{d}</div>
              </div>
            ))}
          </div>
        </div>
        <div className="relative text-[11px] text-[#5B6478]">只对受邀成员开放</div>
      </div>
      <div className="grid place-items-center px-6 py-10">
      <form onSubmit={submit} className="w-full max-w-[360px] animate-rise">
        <div className="mb-6">
          <span className="ico ico-brand w-10 h-10 mb-4"><Lock size={18} strokeWidth={1.8} /></span>
          <div className="text-[22px] font-semibold tracking-tight">{tab === "register" ? "注册账号" : "欢迎回来"}</div>
          <p className="text-[13px] text-ink-faint mt-1">这台机器已对外开放，需要本机账号才能进入</p>
        </div>
        {notice && <p className="flex items-start gap-2 text-aux text-warn bg-warn-50 border border-warn/15 rounded-ctl px-3 py-2.5 mb-5" role="alert"><AlertTriangle size={14} className="shrink-0 mt-0.5" /><span data-testid="auth-notice">{notice}</span></p>}
        <div className="segmented mb-6" role="tablist">
          <button type="button" role="tab" aria-pressed={tab === "login"} onClick={() => setTab("login")}>登录</button>
          <button type="button" role="tab" aria-pressed={tab === "register"} onClick={() => setTab("register")}>注册</button>
        </div>
        <label className="label" htmlFor="auth-user">用户名</label>
        <input id="auth-user" className="field h-10 mb-4" value={u} onChange={(e) => setU(e.target.value)} placeholder="用户名" autoComplete="username" autoFocus />
        <label className="label" htmlFor="auth-pass">密码</label>
        <input id="auth-pass" className="field h-10 mb-4" type="password" value={p} onChange={(e) => setP(e.target.value)}
          placeholder={tab === "register" ? "至少 8 位" : "密码"} autoComplete={tab === "register" ? "new-password" : "current-password"} />
        {tab === "register" && (
          <>
            <label className="label" htmlFor="auth-invite">邀请码</label>
            <input id="auth-invite" className="field h-10 font-mono" value={invite} onChange={(e) => setInvite(e.target.value)} placeholder="找启动这台机器的人要" autoComplete="off" />
            <p className="hint mb-4">管理员邀请码注册为管理员；普通邀请码注册为成员，GPU 配额默认为 0，需管理员分配。</p>
          </>
        )}
        <button type="submit" className="btn btn-pri w-full h-10 mt-2 text-[14px]" disabled={busy}>
          {busy ? "处理中…" : tab === "register" ? "注册并登录" : "登录"}
        </button>
        <p className="text-[11px] text-ink-faint mt-6 pt-4 border-t border-line-soft">本机设备指纹 <span className="font-mono select-all" data-testid="device-id">{deviceId}</span></p>
      </form>
      </div>
    </div>
  );
}
