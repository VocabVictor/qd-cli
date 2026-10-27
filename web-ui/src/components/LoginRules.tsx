import { useEffect, useState } from "react";
import { Loader2, Save, LogOut, RefreshCw, ShieldCheck } from "lucide-react";
import { api, BindKind, ListMode, LiveSession, LoginOverride, LoginPolicy } from "../lib/api";
import { Card, Panel, Th, Td, useToast } from "./ui";
import { ReactNode } from "react";

/** 面板里的表格：不再套一层卡片。 */
function TableBare({ head, children }: { head: ReactNode; children: ReactNode }) {
  return <div className="overflow-x-auto"><table className="tbl"><thead><tr>{head}</tr></thead><tbody>{children}</tbody></table></div>;
}

const MODES: [LoginPolicy["sessionMode"], string, string][] = [
  ["kick", "踢掉旧会话", "新登录把其他地方的登录挤下线"],
  ["deny", "拒绝新登录", "已在别处使用时不让登录，闲置 30 分钟后放行"],
  ["off", "不限制", "允许同时在多处登录"],
];
const BINDS: [BindKind, string, string][] = [
  ["ip", "IP 地址", "IP 一变就退出，最严格；手机网络、代理切换会频繁掉线"],
  ["subnet", "网段", "同一 /24（IPv6 为 /64）内换 IP 不掉线"],
  ["agent", "浏览器", "User-Agent 变了就退出，挡住 Cookie 被拷到别的浏览器"],
  ["device", "设备指纹", "浏览器算出的设备指纹变了就退出"],
];
export const DEFAULT_POLICY: LoginPolicy = { sessionMode: "kick", bind: ["agent", "device"], ipAllow: [], deviceAllow: [] };

const lines = (text: string) => text.split(/[\n,，\s]+/).map((t) => t.trim()).filter(Boolean);

/** 登录限制编辑器：单点登录方式、会话绑定项、IP 与设备白名单。 */
export function PolicyEditor({ value, onChange, disabled }: { value: LoginPolicy; onChange: (p: LoginPolicy) => void; disabled?: boolean }) {
  const [ipText, setIpText] = useState(value.ipAllow.join("\n"));
  const [devText, setDevText] = useState(value.deviceAllow.join("\n"));
  useEffect(() => { setIpText(value.ipAllow.join("\n")); setDevText(value.deviceAllow.join("\n")); }, [value.ipAllow.join(), value.deviceAllow.join()]); // eslint-disable-line
  const toggle = (k: BindKind) => onChange({ ...value, bind: value.bind.includes(k) ? value.bind.filter((b) => b !== k) : [...value.bind, k] });

  return (
    <fieldset disabled={disabled} className="grid md:grid-cols-2 gap-x-8 gap-y-4 disabled:opacity-60">
      <div>
        <div className="text-aux font-semibold mb-2">单点登录</div>
        {MODES.map(([k, label, hint]) => (
          <label key={k} className="flex items-start gap-2 py-1 cursor-pointer">
            <input type="radio" className="accent-brand mt-0.5" checked={value.sessionMode === k} onChange={() => onChange({ ...value, sessionMode: k })} />
            <span><span className="text-[13px]">{label}</span><span className="block text-[11px] text-ink-faint">{hint}</span></span>
          </label>
        ))}
      </div>
      <div>
        <div className="text-aux font-semibold mb-2">会话绑定（登录后这些变了就强制退出）</div>
        {BINDS.map(([k, label, hint]) => (
          <label key={k} className="flex items-start gap-2 py-1 cursor-pointer">
            <input type="checkbox" className="accent-brand mt-0.5" checked={value.bind.includes(k)} onChange={() => toggle(k)} />
            <span><span className="text-[13px]">{label}</span><span className="block text-[11px] text-ink-faint">{hint}</span></span>
          </label>
        ))}
      </div>
      <label className="block">
        <span className="text-aux font-semibold">IP 白名单</span>
        <span className="block text-[11px] text-ink-faint mb-1">一行一个 IP 或网段，如 10.8.0.0/16；留空表示不限</span>
        <textarea className="field h-24 font-mono text-[12px] py-1.5" value={ipText} aria-label="IP 白名单"
          onChange={(e) => setIpText(e.target.value)} onBlur={() => onChange({ ...value, ipAllow: lines(ipText) })} />
      </label>
      <label className="block">
        <span className="text-aux font-semibold">设备白名单</span>
        <span className="block text-[11px] text-ink-faint mb-1">一行一个设备指纹（登录页底部可以看到）；留空表示不限</span>
        <textarea className="field h-24 font-mono text-[12px] py-1.5" value={devText} aria-label="设备白名单"
          onChange={(e) => setDevText(e.target.value)} onBlur={() => onChange({ ...value, deviceAllow: lines(devText) })} />
      </label>
    </fieldset>
  );
}

/** 全局登录限制：所有成员的默认规则，单个成员可以逐项覆盖。 */
export function GlobalRules({ onSaved }: { onSaved?: (p: LoginPolicy) => void }) {
  const toast = useToast();
  const [saved, setSaved] = useState<LoginPolicy | null>(null);
  const [draft, setDraft] = useState<LoginPolicy>(DEFAULT_POLICY);
  const [busy, setBusy] = useState(false);
  useEffect(() => { api.admin.policy().then((d) => { setSaved(d.policy); setDraft(d.policy); onSaved?.(d.policy); }).catch((e) => toast(e.message, true)); }, []); // eslint-disable-line
  const dirty = !!saved && JSON.stringify(saved) !== JSON.stringify(draft);
  const save = async () => {
    setBusy(true);
    try { const d: any = await api.admin.setPolicy(draft); setSaved(d.policy); setDraft(d.policy); onSaved?.(d.policy); toast("登录限制已保存，立即生效"); }
    catch (e: any) { toast(e.message, true); } finally { setBusy(false); }
  };
  return (
    <Card className="p-6">
      <div className="flex items-center gap-3 mb-5">
        <span className="ico ico-teal"><ShieldCheck size={16} /></span>
        <div>
          <div className="text-[15px] font-semibold">成员登录限制（全体）</div>
          <div className="text-[11px] text-ink-faint">所有成员的默认规则，管理员不受限；单个成员可以在「用户与配额」的卡片里逐项覆盖。改完立即生效。</div>
        </div>
        <div className="flex-1" />
        <button className="btn btn-pri" disabled={busy || !dirty} onClick={save} data-testid="save-policy">
          {busy ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}保存</button>
      </div>
      {saved ? <PolicyEditor value={draft} onChange={setDraft} /> : <p className="text-aux text-ink-faint">加载中…</p>}
    </Card>
  );
}

const ago = (t: number) => {
  const s = Math.max(0, Math.floor(Date.now() / 1000 - t));
  return s < 60 ? "刚刚" : s < 3600 ? `${Math.floor(s / 60)} 分钟前` : s < 86400 ? `${Math.floor(s / 3600)} 小时前` : `${Math.floor(s / 86400)} 天前`;
};
const browserOf = (ua: string) => (/Edg\//.test(ua) ? "Edge" : /Chrome\//.test(ua) ? "Chrome" : /Firefox\//.test(ua) ? "Firefox" : /Safari\//.test(ua) ? "Safari" : ua.slice(0, 24) || "—");

/** 在线会话：谁在哪个 IP / 浏览器 / 设备上登录着，可以踢下线。 */
export function LiveSessions({ me }: { me?: string }) {
  const toast = useToast();
  const [list, setList] = useState<LiveSession[] | null>(null);
  const load = () => api.admin.sessions().then((d) => setList(d.sessions)).catch((e) => toast(e.message, true));
  useEffect(() => { load(); }, []); // eslint-disable-line
  const kick = async (s: LiveSession) => {
    if (!confirm(`把 ${s.username}（${s.ip}）踢下线？`)) return;
    try { await api.admin.kick(s.id); toast(`${s.username} 已下线`); load(); } catch (e: any) { toast(e.message, true); }
  };
  return (
    <Panel title="在线会话" meta={list ? `${list.length} 个会话，管理员可以把成员踢下线` : undefined} action={<button className="btn btn-mini" onClick={load}><RefreshCw size={12} />刷新</button>}>
      <TableBare head={<><Th>用户</Th><Th>IP</Th><Th>浏览器</Th><Th>设备指纹</Th><Th>登录于</Th><Th>最近活动</Th><Th /></>}>
        {(list || []).map((s) => (
          <tr key={s.id} data-session={s.username}>
            <Td><span className="font-medium">{s.username}</span>{s.current ? <span className="tag tag-brand ml-1.5">当前会话</span> : me && s.username.toLowerCase() === me.toLowerCase() && <span className="tag tag-gray ml-1.5">我</span>}</Td>
            <Td><span className="font-mono text-[12px]">{s.ip}</span></Td>
            <Td><span title={s.agent}>{browserOf(s.agent)}</span></Td>
            <Td><span className="font-mono text-[12px] text-ink-soft select-all">{s.device || "—"}</span></Td>
            <Td className="text-ink-soft whitespace-nowrap">{new Date(s.createdAt * 1000).toLocaleString("zh-CN", { hour12: false })}</Td>
            <Td><span className="inline-flex items-center gap-1.5"><i className={"w-1.5 h-1.5 rounded-full " + (s.online ? "bg-ok" : "bg-ink-faint/50")} /><span className={s.online ? "" : "text-ink-faint"}>{ago(s.seenAt)}</span></span></Td>
            <Td className="text-right">{s.current
              ? <span className="text-[11px] text-ink-faint">退出请用左下角</span>
              : <button className="btn btn-mini btn-danger" onClick={() => kick(s)}><LogOut size={12} />踢下线</button>}</Td>
          </tr>
        ))}
        {list && !list.length && <tr><td colSpan={7} className="px-4 py-10 text-center text-ink-faint">没有在线会话</td></tr>}
      </TableBare>
    </Panel>
  );
}

export const EMPTY_OVERRIDE: LoginOverride = {
  exempt: false, sessionMode: null, bind: null,
  ipAllow: { mode: "inherit", items: [] }, deviceAllow: { mode: "inherit", items: [] },
};
const LIST_MODES: [ListMode, string][] = [["inherit", "跟随全局"], ["extend", "在全局上追加"], ["replace", "单独设置"]];
const modeName = (m: LoginPolicy["sessionMode"]) => MODES.find(([k]) => k === m)?.[1] || m;
const bindName = (b: BindKind[]) => (b.length ? b.map((k) => BINDS.find(([x]) => x === k)?.[1]).join("、") : "不绑定");
const listName = (l: string[]) => (l.length ? `${l.length} 条` : "不限");

function Follow({ on, onChange, label }: { on: boolean; onChange: (follow: boolean) => void; label: string }) {
  return (
    <div className="segmented w-40" role="group" aria-label={`${label}来源`}>
      <button aria-pressed={on} onClick={() => onChange(true)}>跟随全局</button>
      <button aria-pressed={!on} onClick={() => onChange(false)}>单独设置</button>
    </div>
  );
}

type ListValue = LoginOverride["ipAllow"];

function ListRule({ label, hint, value, global, onChange }: {
  label: string; hint: string; value: ListValue; global: string[]; onChange: (v: ListValue) => void;
}) {
  const [text, setText] = useState(value.items.join("\n"));
  useEffect(() => setText(value.items.join("\n")), [value.items.join()]); // eslint-disable-line
  return (
    <div>
      <div className="flex items-center gap-2 mb-1.5">
        <span className="text-aux font-semibold w-20">{label}</span>
        <select className="field field-sm w-36" aria-label={`${label}方式`} value={value.mode} onChange={(e) => onChange({ ...value, mode: e.target.value as ListMode })}>
          {LIST_MODES.map(([k, l]) => <option key={k} value={k}>{l}</option>)}
        </select>
        <span className="text-[11px] text-ink-faint truncate">全局：{global.length ? global.join("、") : "不限"}</span>
      </div>
      {value.mode !== "inherit" && (
        <>
          <textarea className="field h-20 font-mono text-[12px] py-1.5 bg-white" value={text} aria-label={`${label}条目`}
            onChange={(e) => setText(e.target.value)} onBlur={() => onChange({ ...value, items: lines(text) })} />
          <p className="text-[11px] text-ink-faint mt-1">
            {value.mode === "extend"
              ? (global.length ? "在全局白名单之外，再额外允许这些。" : "全局没有限制，追加的条目暂时不起作用。")
              : `只认这里的条目，${hint}；留空表示这个成员不限制。`}
          </p>
        </>
      )}
    </div>
  );
}

/** 单个成员的登录限制：每一项都能选跟随全局或单独设置，白名单还能在全局基础上追加。 */
export function UserOverride({ value, global, effective, onChange }: {
  value: LoginOverride; global: LoginPolicy | null; effective?: LoginPolicy | null; onChange: (v: LoginOverride) => void;
}) {
  const g = global || DEFAULT_POLICY;
  const set = (patch: Partial<LoginOverride>) => onChange({ ...value, ...patch });
  const toggle = (k: BindKind) => {
    const cur = value.bind || g.bind;
    set({ bind: cur.includes(k) ? cur.filter((b) => b !== k) : [...cur, k] });
  };
  return (
    <div className="space-y-4">
      <label className="flex items-center gap-2 cursor-pointer">
        <input type="checkbox" className="accent-brand" checked={value.exempt} onChange={(e) => set({ exempt: e.target.checked })} />
        <span className="text-[13px]">免除登录限制</span>
        <span className="text-[11px] text-ink-faint">像管理员一样，不受单点登录、绑定和白名单约束</span>
      </label>
      {!value.exempt && (
        <div className="grid md:grid-cols-2 gap-x-8 gap-y-4">
          <div>
            <div className="flex items-center gap-2 mb-1.5">
              <span className="text-aux font-semibold w-20">单点登录</span>
              <Follow label="单点登录" on={value.sessionMode === null} onChange={(f) => set({ sessionMode: f ? null : g.sessionMode })} />
            </div>
            {value.sessionMode === null ? <p className="text-[11px] text-ink-faint">全局：{modeName(g.sessionMode)}</p> : MODES.map(([k, label]) => (
              <label key={k} className="flex items-center gap-2 py-0.5 cursor-pointer text-[13px]">
                <input type="radio" className="accent-brand" checked={value.sessionMode === k} onChange={() => set({ sessionMode: k })} />{label}
              </label>
            ))}
          </div>
          <div>
            <div className="flex items-center gap-2 mb-1.5">
              <span className="text-aux font-semibold w-20">会话绑定</span>
              <Follow label="会话绑定" on={value.bind === null} onChange={(f) => set({ bind: f ? null : [...g.bind] })} />
            </div>
            {value.bind === null ? <p className="text-[11px] text-ink-faint">全局：{bindName(g.bind)}</p> : (
              <div className="flex flex-wrap gap-x-4 gap-y-1">
                {BINDS.map(([k, label]) => (
                  <label key={k} className="flex items-center gap-1.5 cursor-pointer text-[13px]">
                    <input type="checkbox" className="accent-brand" checked={value.bind!.includes(k)} onChange={() => toggle(k)} />{label}
                  </label>
                ))}
              </div>
            )}
          </div>
          <ListRule label="IP 白名单" hint="一行一个 IP 或网段" value={value.ipAllow} global={g.ipAllow} onChange={(ipAllow) => set({ ipAllow })} />
          <ListRule label="设备白名单" hint="一行一个设备指纹" value={value.deviceAllow} global={g.deviceAllow} onChange={(deviceAllow) => set({ deviceAllow })} />
        </div>
      )}
      <p className="text-[11px] text-ink-faint" data-testid="effective">
        实际生效：{effective === null ? "不受限制" : effective
          ? `${modeName(effective.sessionMode)}；绑定 ${bindName(effective.bind)}；IP 白名单 ${listName(effective.ipAllow)}；设备白名单 ${listName(effective.deviceAllow)}`
          : "保存后显示"}
      </p>
    </div>
  );
}
