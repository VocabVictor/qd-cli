import { useEffect, useState, useMemo } from "react";
import { Loader2 } from "lucide-react";
import { api, qpost, pcall, eachSpace, spacesOf, AppState, asList, field } from "../lib/api";
import { Drawer } from "./Drawer";
import { useToast } from "./ui";

type Kind = "job" | "dev";

export function CreateForm({ kind, state, open, onClose, onDone }: { kind: Kind; state: AppState; open: boolean; onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const spaces = spacesOf(state);
  const [space, setSpace] = useState(state.spaceId || spaces[0]?.id || "");
  const [name, setName] = useState(kind === "job" ? "qd-job" : "qd-dev");
  const [projects, setProjects] = useState<any[]>([]);
  const [groups, setGroups] = useState<any[]>([]);
  const [repos, setRepos] = useState<any[]>([]);
  const [tags, setTags] = useState<any[]>([]);
  const [projectId, setProjectId] = useState("");
  const [rsgroupId, setRsgroupId] = useState("");
  const [repoId, setRepoId] = useState("");
  const [imageId, setImageId] = useState("");
  const [useGpu, setUseGpu] = useState(kind === "job");
  const [gpuType, setGpuType] = useState("");
  const [gpu, setGpu] = useState(1);
  const [cpu, setCpu] = useState(8);
  const [memory, setMemory] = useState(32);
  const [storage, setStorage] = useState(100);
  const [runScript, setRunScript] = useState("python /gemini/code/main.py");
  const [maxRunHour, setMaxRunHour] = useState(24);
  const [advanced, setAdvanced] = useState(false);
  const [jsonText, setJsonText] = useState("");
  const [busy, setBusy] = useState(false);
  const [loadingOpts, setLoadingOpts] = useState(true);

  useEffect(() => {
    if (!open) return;
    setLoadingOpts(true);
    (async () => {
      const oneSpace = { ...state, spaces: spaces.filter((s) => s.id === space) } as AppState;
      const [prjPer, idle] = await Promise.all([
        eachSpace(oneSpace, (sp) => pcall("GET", "core", "/project/list", { pageNum: 1, pageSize: 500, accessType: 1 }, sp.id)).catch(() => [] as any[]),
        api.idle().catch(() => ({ groups: [] })),
      ]);
      const prj = (prjPer as any[]).flatMap(({ data }: any) => (data.data?.projectList || data.data?.list || []));
      const grp = (idle.groups || []).filter((g: any) => !g.spaceId || g.spaceId === space);
      setProjects(prj); setGroups(grp);
      if (prj[0]) setProjectId(String(field(prj[0], ["projectId", "id"], "")));
      if (grp[0]) setRsgroupId(grp[0].rsgroupId);
      setLoadingOpts(false);
    })();
  }, [open, space]); // eslint-disable-line

  useEffect(() => {
    if (!open) return;
    pcall("GET", "core", "/imageRepository/list", { spaceId: space, source: "current", keyWords: "", pageNum: 1, pageSize: 500 }, space)
      .then((d) => setRepos(d.data?.imageList || [])).catch(() => setRepos([]));
  }, [open, space]);

  useEffect(() => {
    if (!repoId) { setTags([]); return; }
    pcall("GET", "core", "/imageRepository/detail/" + repoId, {}, space)
      .then((d) => { const l = d.data?.imageList || []; setTags(l); if (l[0]) setImageId(String(field(l[0], ["imageId"], ""))); })
      .catch(() => setTags([]));
  }, [repoId, space]);

  const gpuTypes = useMemo(() => {
    const g = groups.find((x) => x.rsgroupId === rsgroupId);
    return asList(g?.capacity?.gpuTypes);
  }, [groups, rsgroupId]);
  useEffect(() => { if (gpuTypes[0] && !gpuType) setGpuType(gpuTypes[0]); }, [gpuTypes]); // eslint-disable-line

  const payload = useMemo(() => {
    const spec = { cpu, memory: memory * 1024, storage: storage * 1024, gpu: useGpu ? gpu : 0, gpuType: useGpu ? gpuType : "" };
    if (kind === "job") {
      return {
        jobName: name, trainType: 1, spaceId: space, projectId, rsgroupId,
        description: "submitted by qd web", imageId: Number(imageId) || 0, maxRunHour, maxRetryCount: 0,
        datasetInData: [], preInData: { dataType: 0, dataPath: "", dataBucket: "" }, services: [],
        taskroles: [{ instance: 1, runScript, customEnv: "", ...spec }],
      };
    }
    return { jobenvName: name, spaceId: space, projectId, rsgroupId, imageId: Number(imageId) || 0, ...spec };
  }, [kind, name, space, projectId, rsgroupId, imageId, useGpu, gpu, gpuType, cpu, memory, storage, runScript, maxRunHour]);

  useEffect(() => { setJsonText(JSON.stringify(payload, null, 2)); }, [payload]);

  const submit = async () => {
    let body: any;
    try { body = advanced ? JSON.parse(jsonText) : payload; }
    catch (e: any) { return toast("JSON 语法错误：" + e.message, true); }
    if (!body.projectId) return toast("请选择项目", true);
    if (!body.rsgroupId) return toast("请选择资源组", true);
    setBusy(true);
    try {
      if (kind === "job") { const r = await qpost("/api/job/submit", body); toast("已提交作业：" + ((r.data?.jobId) || "成功")); }
      else { await pcall("POST", "core", "/jobenv/devJob/new", body); toast("已创建开发机"); }
      onDone(); onClose();
    } catch (e: any) { toast(e.message, true); }
    finally { setBusy(false); }
  };

  const L = ({ children }: { children: any }) => <label className="block text-xs font-medium text-ink-soft mb-1 mt-3">{children}</label>;

  return (
    <Drawer open={open} onClose={onClose} title={kind === "job" ? "提交作业" : "申请开发机"}>
      {loadingOpts ? <div className="flex items-center gap-2 text-ink-faint text-sm py-10 justify-center"><Loader2 size={16} className="animate-spin" />加载选项…</div> : (
        <>
          {!advanced ? (
            <div className="grid grid-cols-2 gap-x-4">
              <div><L>名称</L><input className="field" value={name} onChange={(e) => setName(e.target.value)} /></div>
              <div><L>空间</L><select className="field" value={space} onChange={(e) => setSpace(e.target.value)}>{spaces.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}</select></div>
              <div><L>项目</L><select className="field" value={projectId} onChange={(e) => setProjectId(e.target.value)}>{projects.length ? projects.map((p) => { const id = String(field(p, ["projectId", "id"], "")); return <option key={id} value={id}>{field(p, ["projectName", "name"])}</option>; }) : <option value="">无项目</option>}</select></div>
              <div><L>资源组</L><select className="field" value={rsgroupId} onChange={(e) => setRsgroupId(e.target.value)}>{groups.map((g) => <option key={g.rsgroupId} value={g.rsgroupId}>{g.rsgroupName || g.rsgroupId}</option>)}</select></div>
              <div><L>镜像仓库</L><select className="field" value={repoId} onChange={(e) => setRepoId(e.target.value)}><option value="">选择仓库</option>{repos.map((r) => <option key={r.imageRepositoryId} value={r.imageRepositoryId}>{field(r, ["originalName", "repositoryName"])}</option>)}</select></div>
              <div><L>镜像 Tag</L><select className="field" value={imageId} onChange={(e) => setImageId(e.target.value)}>{tags.length ? tags.map((t) => <option key={field(t, ["imageId"])} value={String(field(t, ["imageId"]))}>{field(t, ["imageTag"])}</option>) : <option value="">先选仓库</option>}</select></div>
              <div className="col-span-2"><L>算力类型</L>
                <div className="flex gap-2">
                  <button className={"chip " + (!useGpu ? "chip-on" : "")} onClick={() => setUseGpu(false)}>CPU</button>
                  <button className={"chip " + (useGpu ? "chip-on" : "")} onClick={() => setUseGpu(true)}>GPU</button>
                </div>
              </div>
              {useGpu && <div><L>GPU 型号</L><select className="field" value={gpuType} onChange={(e) => setGpuType(e.target.value)}>{gpuTypes.length ? gpuTypes.map((t) => <option key={t} value={t}>{t}</option>) : <option value="">该资源组无 GPU</option>}</select></div>}
              {useGpu && <div><L>GPU 卡数</L><input className="field" type="number" min={1} value={gpu} onChange={(e) => setGpu(Number(e.target.value))} /></div>}
              <div><L>CPU（核）</L><input className="field" type="number" min={1} value={cpu} onChange={(e) => setCpu(Number(e.target.value))} /></div>
              <div><L>内存（GiB）</L><input className="field" type="number" min={1} value={memory} onChange={(e) => setMemory(Number(e.target.value))} /></div>
              <div><L>存储（GiB）</L><input className="field" type="number" min={1} value={storage} onChange={(e) => setStorage(Number(e.target.value))} /></div>
              {kind === "job" && <div><L>最长运行（小时）</L><input className="field" type="number" min={1} value={maxRunHour} onChange={(e) => setMaxRunHour(Number(e.target.value))} /></div>}
              {kind === "job" && <div className="col-span-2"><L>运行命令</L><input className="field" value={runScript} onChange={(e) => setRunScript(e.target.value)} /></div>}
            </div>
          ) : (
            <><L>提交 JSON（可编辑）</L><textarea className="field font-mono text-xs" style={{ height: 360, resize: "vertical", paddingTop: 8 }} value={jsonText} onChange={(e) => setJsonText(e.target.value)} /></>
          )}
          <div className="flex items-center gap-2 mt-5">
            <label className="flex items-center gap-1.5 text-sm text-ink-soft cursor-pointer"><input type="checkbox" checked={advanced} onChange={(e) => setAdvanced(e.target.checked)} />高级 JSON</label>
            <div className="flex-1" />
            <button className="btn" onClick={onClose}>取消</button>
            <button className="btn btn-pri" onClick={submit} disabled={busy}>{busy && <Loader2 size={14} className="animate-spin" />}{kind === "job" ? "提交作业" : "创建开发机"}</button>
          </div>
        </>
      )}
    </Drawer>
  );
}
