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
    <div className="h-full grid place-items-center px-4 bg-canvas">
      <form onSubmit={submit} className="card w-full max-w-[340px] p-7">
        <div className="flex flex-col items-center text-center mb-5">
          <span className="grid place-items-center w-9 h-9 rounded-[7px] bg-ink text-white mb-3"><Zap size={17} strokeWidth={1.8} /></span>
          <div className="text-[17px] font-semibold">登录 GPU 平台</div>
          <p className="text-aux text-ink-faint mt-1">这台机器上的 qd 将用这个平台账号代所有人提交作业</p>
        </div>
        <label className="block text-aux font-medium text-ink-soft mb-1" htmlFor="pf-user">平台账号</label>
        <input id="pf-user" className="field mb-3" value={u} onChange={(e) => setU(e.target.value)} placeholder="用户名" autoComplete="username" autoFocus />
        <label className="block text-aux font-medium text-ink-soft mb-1" htmlFor="pf-pass">密码</label>
        <input id="pf-pass" className="field mb-4" type="password" value={p} onChange={(e) => setP(e.target.value)} placeholder="密码" autoComplete="current-password" />
        <div className="flex items-center gap-4 mb-5 text-[13px] text-ink-soft">
          <label className="flex items-center gap-1.5 cursor-pointer"><input type="checkbox" className="accent-brand" checked={ldap} onChange={(e) => setLdap(e.target.checked)} /> LDAP</label>
          <label className="flex items-center gap-1.5 cursor-pointer"><input type="checkbox" className="accent-brand" checked={remember} onChange={(e) => setRemember(e.target.checked)} /> 记住登录</label>
        </div>
        <button type="submit" className="btn btn-pri w-full h-9" disabled={busy}>{busy ? "登录中…" : "登录"}</button>
      </form>
    </div>
  );
}
