import { useState } from "react";
import { Zap } from "lucide-react";
import { api } from "../lib/api";
import { useToast } from "../components/ui";

export function Login({ onLogin }: { onLogin: () => void }) {
  const toast = useToast();
  const [u, setU] = useState("");
  const [p, setP] = useState("");
  const [ldap, setLdap] = useState(false);
  const [remember, setRemember] = useState(true);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!u || !p) return toast("请输入账号和密码", true);
    setBusy(true);
    try { await api.login(u, p, ldap, remember); onLogin(); }
    catch (err: any) { toast(err.message || "登录失败", true); }
    finally { setBusy(false); }
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
          <span className="ico ico-teal w-10 h-10 mb-4"><Zap size={18} strokeWidth={1.8} /></span>
          <div className="text-[22px] font-semibold tracking-tight">登录 GPU 平台</div>
          <p className="text-[13px] text-ink-faint mt-1">这台机器上的 qd 将用这个平台账号代所有人提交作业</p>
        </div>
        <label className="label" htmlFor="pf-user">平台账号</label>
        <input id="pf-user" className="field h-10 mb-4" value={u} onChange={(e) => setU(e.target.value)} placeholder="用户名" autoComplete="username" autoFocus />
        <label className="label" htmlFor="pf-pass">密码</label>
        <input id="pf-pass" className="field h-10 mb-4" type="password" value={p} onChange={(e) => setP(e.target.value)} placeholder="密码" autoComplete="current-password" />
        <div className="flex items-center gap-4 mb-5 text-[13px] text-ink-soft">
          <label className="flex items-center gap-1.5 cursor-pointer"><input type="checkbox" className="accent-brand" checked={ldap} onChange={(e) => setLdap(e.target.checked)} /> LDAP</label>
          <label className="flex items-center gap-1.5 cursor-pointer"><input type="checkbox" className="accent-brand" checked={remember} onChange={(e) => setRemember(e.target.checked)} /> 记住登录</label>
        </div>
        <button type="submit" className="btn btn-pri w-full h-10 text-[14px]" disabled={busy}>{busy ? "登录中…" : "登录"}</button>
      </form>
      </div>
    </div>
  );
}
