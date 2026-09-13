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
    <div className="h-full grid place-items-center px-4">
      <form onSubmit={submit} className="card w-full max-w-sm p-7">
        <div className="flex items-center gap-2.5 mb-1">
          <span className="grid place-items-center w-9 h-9 rounded-xl text-white" style={{ background: "linear-gradient(135deg,#7c5cfc,#b14eff)" }}><Zap size={19} /></span>
          <div className="text-lg font-semibold">qd 控制台</div>
        </div>
        <p className="text-sm text-ink-faint mb-6">登录 GPU 平台</p>
        <label className="block text-xs font-medium text-ink-soft mb-1">账号</label>
        <input className="field mb-3" value={u} onChange={(e) => setU(e.target.value)} placeholder="用户名" autoFocus />
        <label className="block text-xs font-medium text-ink-soft mb-1">密码</label>
        <input className="field mb-4" type="password" value={p} onChange={(e) => setP(e.target.value)} placeholder="密码" />
        <div className="flex items-center gap-4 mb-5 text-sm text-ink-soft">
          <label className="flex items-center gap-1.5 cursor-pointer"><input type="checkbox" checked={ldap} onChange={(e) => setLdap(e.target.checked)} /> LDAP</label>
          <label className="flex items-center gap-1.5 cursor-pointer"><input type="checkbox" checked={remember} onChange={(e) => setRemember(e.target.checked)} /> 记住登录</label>
        </div>
        <button className="btn btn-pri w-full justify-center" disabled={busy}>{busy ? "登录中…" : "登录"}</button>
      </form>
    </div>
  );
}
