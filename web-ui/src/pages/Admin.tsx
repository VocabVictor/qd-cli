import { useEffect, useMemo, useState } from "react";
import { RefreshCw, Loader2, Trash2, KeyRound, Save } from "lucide-react";
import { api, AppState, AdminUser, GpuLimits, LoginOverride, LoginPolicy, getMe, spacesOf } from "../lib/api";
import { EMPTY_OVERRIDE, GlobalRules, LiveSessions, UserOverride } from "../components/LoginRules";
import { PageBar, Card, Spinner, useToast } from "../components/ui";
import { Meter } from "../components/viz";

type Group = { id: string; name: string; spaceId: string; spaceName: string };

const num = (v: string) => Math.max(0, Math.min(4096, Math.floor(Number(v) || 0)));

function LimitInput({ value, used, onChange, label }: { value: number; used: number; onChange: (n: number) => void; label: string }) {
  return (
    <label className="flex items-center gap-2 text-aux">
      <span className="w-40 truncate text-ink-soft" title={label}>{label}</span>
      <input className="field w-20 text-right tabular-nums" type="number" min={0} max={4096} value={value}
        onChange={(e) => onChange(num(e.target.value))} aria-label={`${label} 上限`} />
      <span className="text-ink-faint tabular-nums w-16">已用 {used}</span>
    </label>
  );
}

/** 单个成员的登录限制：每一项可以跟随全局，也可以单独设置。 */
function UserRules({ user, global, onChanged }: { user: AdminUser; global: LoginPolicy | null; onChanged: () => void }) {
  const toast = useToast();
  const saved = user.login || EMPTY_OVERRIDE;
  const [draft, setDraft] = useState<LoginOverride>(saved);
  const [open, setOpen] = useState(!!user.login);
  const [busy, setBusy] = useState(false);
  useEffect(() => { setDraft(user.login || EMPTY_OVERRIDE); }, [user]);
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const save = async (value: LoginOverride | null) => {
    setBusy(true);
    try { await api.admin.setUserLogin(user.username, value); toast(`${user.username} 的登录限制已保存`); onChanged(); }
    catch (e: any) { toast(e.message, true); } finally { setBusy(false); }
  };
  return (
    <div className="mt-4 pt-4 border-t border-line-soft">
      <div className="flex items-center gap-3 mb-3">
        <button className="text-aux font-semibold hover:text-brand" onClick={() => setOpen((v) => !v)} aria-expanded={open}>
          登录限制 {open ? "▾" : "▸"}</button>
        <span className="text-[11px] text-ink-faint">{user.login ? (user.login.exempt ? "已免除限制" : "有单独设置") : "全部跟随全局"}</span>
        <div className="flex-1" />
        {user.login && <button className="btn btn-mini" disabled={busy} onClick={() => save(null)}>全部恢复跟随全局</button>}
        {open && <button className="btn btn-mini btn-pri" disabled={busy || !dirty} onClick={() => save(draft)}><Save size={12} />保存登录限制</button>}
      </div>
      {open && <UserOverride value={draft} global={global} effective={dirty ? undefined : user.effective} onChange={setDraft} />}
    </div>
  );
}

function UserRow({ user, spaces, groups, global, onChanged }: { user: AdminUser; spaces: { id: string; name: string }[]; groups: Group[]; global: LoginPolicy | null; onChanged: () => void }) {
  const toast = useToast();
  const [quota, setQuota] = useState<GpuLimits>(user.quota);
  const [busy, setBusy] = useState(false);
  const self = user.username.toLowerCase() === (getMe().username || "").toLowerCase();
  const admin = user.role === "admin";
  const dirty = JSON.stringify(quota) !== JSON.stringify(user.quota);
  useEffect(() => setQuota(user.quota), [user]);

  const run = async (fn: () => Promise<any>, ok: string) => {
    setBusy(true);
    try { await fn(); toast(ok); onChanged(); } catch (e: any) { toast(e.message, true); } finally { setBusy(false); }
  };
  const setDim = (dim: "spaces" | "groups", id: string, n: number) =>
    setQuota((q) => ({ ...q, [dim]: { ...q[dim], [id]: n } }));

  return (
    <div data-user={user.username}>
    <Card className="p-5">
      <div className="flex items-center gap-3 mb-4">
        <span className="grid place-items-center w-9 h-9 rounded-full bg-black/[0.06] font-semibold text-ink-soft uppercase">{user.username.slice(0, 1)}</span>
        <div className="min-w-0">
          <div className="text-[15px] font-semibold flex items-center gap-2">{user.username}
            <span className={"tag " + (admin ? "tag-violet" : "tag-gray")}>{admin ? "管理员" : "成员"}</span>
            {self && <span className="tag tag-brand">我</span>}
          </div>
          <div className="text-[11px] text-ink-faint">注册于 {user.createdAt ? new Date(user.createdAt * 1000).toLocaleString("zh-CN", { hour12: false }) : "-"}</div>
        </div>
        <div className="flex-1" />
        <div className="segmented w-36">
          <button aria-pressed={!admin} disabled={busy} onClick={() => admin && run(() => api.admin.setRole(user.username, "user"), `${user.username} 已设为成员`)}>成员</button>
          <button aria-pressed={admin} disabled={busy} onClick={() => !admin && run(() => api.admin.setRole(user.username, "admin"), `${user.username} 已设为管理员`)}>管理员</button>
        </div>
        <button className="btn btn-mini" disabled={busy} onClick={() => {
          const pw = prompt(`给 ${user.username} 设置新密码（至少 8 位）：`);
          if (pw) run(() => api.admin.resetPassword(user.username, pw), "密码已重置，对方需重新登录");
        }}><KeyRound size={12} />重置密码</button>
        <button className="btn btn-mini btn-danger" disabled={busy || self} title={self ? "不能删除自己" : undefined} onClick={() => {
          if (confirm(`确定删除用户 ${user.username}？其登录会立即失效，已提交的作业不受影响。`)) run(() => api.admin.remove(user.username), "已删除");
        }}><Trash2 size={12} />删除</button>
      </div>

      {admin ? <p className="text-aux text-ink-faint">管理员不受 GPU 配额限制。</p> : (
        <>
          <div className="flex items-center gap-4 mb-4">
            <LimitInput label="总配额（全部空间）" value={quota.total} used={user.usage.total} onChange={(n) => setQuota((q) => ({ ...q, total: n }))} />
            <div className="flex-1 max-w-xs"><Meter value={user.usage.total} total={quota.total || 1} tone={quota.total && user.usage.total >= quota.total ? "danger" : "brand"} /></div>
          </div>
          <div className="grid md:grid-cols-2 gap-x-8 gap-y-2">
            <div>
              <div className="text-aux font-semibold mb-2">按空间</div>
              {spaces.map((s) => <LimitInput key={s.id} label={s.name || s.id} value={quota.spaces[s.id] || 0} used={user.usage.spaces[s.id] || 0} onChange={(n) => setDim("spaces", s.id, n)} />)}
            </div>
            <div>
              <div className="text-aux font-semibold mb-2">按资源组</div>
              {groups.length ? groups.map((g) => <LimitInput key={g.id} label={`${g.name}（${g.spaceName}）`} value={quota.groups[g.id] || 0} used={user.usage.groups[g.id] || 0} onChange={(n) => setDim("groups", g.id, n)} />)
                : <p className="text-aux text-ink-faint">资源组列表加载中或不可用</p>}
            </div>
          </div>
          <p className="text-[11px] text-ink-faint mt-3">提交 GPU 作业时三项同时检查：总数、作业所在空间、作业所在资源组，任一超限都会被拒绝。未设置的空间/资源组按 0 处理。</p>
          <div className="flex justify-end mt-3">
            <button className="btn btn-pri" disabled={busy || !dirty} onClick={() => run(() => api.admin.setQuota(user.username, quota), `${user.username} 的配额已保存`)}>
              {busy ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}保存配额</button>
          </div>
          <UserRules user={user} global={global} onChanged={onChanged} />
        </>
      )}
    </Card>
    </div>
  );
}

export function Admin({ state }: { state: AppState }) {
  const toast = useToast();
  const [users, setUsers] = useState<AdminUser[] | null>(null);
  const [rawGroups, setRawGroups] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [global, setGlobal] = useState<LoginPolicy | null>(null);
  const spaces = spacesOf(state);

  const load = async () => {
    setLoading(true);
    try { setUsers((await api.admin.users()).users); } catch (e: any) { toast(e.message, true); } finally { setLoading(false); }
  };
  useEffect(() => { load(); api.idle().then((d) => setRawGroups(d.groups || [])).catch(() => setRawGroups([])); }, []); // eslint-disable-line

  const groups: Group[] = useMemo(() => {
    const seen = new Set<string>();
    return rawGroups.flatMap((g: any) => {
      const id = String(g.rsgroupId ?? "");
      if (!id || seen.has(id)) return [];
      seen.add(id);
      return [{ id, name: g.rsgroupName || id, spaceId: g.spaceId || "", spaceName: g.spaceName || "" }];
    });
  }, [rawGroups]);

  return (
    <>
      <PageBar title="用户与配额" hint="成员用普通邀请码注册，默认 0 卡；管理员用管理员邀请码注册"
        right={<button className="btn" onClick={load}>{loading ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}刷新</button>} />
      <div className="space-y-3 mb-6">
        <GlobalRules onSaved={setGlobal} />
        <LiveSessions me={getMe().username} />
      </div>
      {!users ? <Spinner /> : (
        <div className="space-y-3">
          {users.map((u) => <UserRow key={u.username} user={u} spaces={spaces} groups={groups} global={global} onChanged={load} />)}
          {!users.length && <Card className="card-pad text-ink-faint">还没有用户</Card>}
        </div>
      )}
    </>
  );
}
