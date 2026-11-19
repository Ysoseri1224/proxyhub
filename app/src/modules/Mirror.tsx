import { useEffect, useState } from "react";
import {
  engineStatus,
  engineDiagnose,
  engineCacheClear,
  engineRunning,
  engineStart,
  engineStop,
  toolPointing,
  engineTakeover,
  engineRestore,
  mirrorAddUpstream,
  mirrorRemoveUpstream,
  mirrorAddEcosystem,
  type ToolPointing,
} from "../api";
import { getErrorMessage, tryParse, formatBytes } from "../util";

interface Upstream {
  name: string;
  base: string;
  up: boolean;
  latency: number;
}
interface Ecosystem {
  name: string;
  prefix: string;
  upstreams: Upstream[];
}
interface Status {
  port: number;
  cacheBytes: number;
  cacheMaxMB: number;
  ecosystems: Ecosystem[];
}
interface RouteEvent {
  time: string;
  eco: string;
  path: string;
  upstream: string;
  cacheHit: boolean;
  status: number;
  latencyMs: number;
}

export function Mirror() {
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState<Status | null>(null);
  const [events, setEvents] = useState<RouteEvent[]>([]);
  const [pointing, setPointing] = useState<ToolPointing[]>([]);
  const [toast, setToast] = useState("");
  const [busy, setBusy] = useState(false);
  // 每个生态的「新增镜像」输入框(name/base)
  const [addUp, setAddUp] = useState<Record<string, { name: string; base: string }>>({});
  // 新增生态表单
  const [newEco, setNewEco] = useState({ name: "", prefix: "", upName: "", upBase: "" });

  async function refresh(): Promise<void> {
    try {
      const up = await engineRunning();
      setRunning(up);
      if (up) {
        setStatus(tryParse<Status>(await engineStatus()));
        const evs = tryParse<RouteEvent[]>(await engineDiagnose());
        if (evs) setEvents(evs.slice(-12).reverse());
      }
      setPointing(await toolPointing());
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  useEffect(() => {
    void refresh();
    const t = setInterval(() => void refresh(), 4000);
    return () => clearInterval(t);
  }, []);

  async function act(fn: () => Promise<string>): Promise<void> {
    try {
      setToast(await fn());
      await refresh();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  // 改镜像源会重启引擎(数秒),期间禁用按钮并提示
  async function mutate(fn: () => Promise<string>): Promise<void> {
    setBusy(true);
    setToast("正在应用并重载引擎…");
    try {
      setToast(await fn());
      await refresh();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  function upDraft(eco: string): { name: string; base: string } {
    return addUp[eco] ?? { name: "", base: "" };
  }
  function setUpDraft(eco: string, patch: Partial<{ name: string; base: string }>): void {
    setAddUp((p) => ({ ...p, [eco]: { ...upDraft(eco), ...patch } }));
  }
  async function submitUpstream(eco: string): Promise<void> {
    const d = upDraft(eco);
    if (!d.name.trim() || !d.base.trim()) return;
    await mutate(() => mirrorAddUpstream(eco, d.name.trim(), d.base.trim()));
    setAddUp((p) => ({ ...p, [eco]: { name: "", base: "" } }));
  }
  async function submitEcosystem(): Promise<void> {
    if (!newEco.name.trim() || !newEco.upName.trim() || !newEco.upBase.trim()) return;
    await mutate(() => mirrorAddEcosystem(newEco.name.trim(), newEco.prefix.trim(), newEco.upName.trim(), newEco.upBase.trim()));
    setNewEco({ name: "", prefix: "", upName: "", upBase: "" });
  }

  const cachePct = status ? (status.cacheBytes / (status.cacheMaxMB * 1024 * 1024)) * 100 : 0;

  return (
    <div>
      <h1 className="page-title">镜像源</h1>
      <p className="page-sub">本地路由代理:多上游池 · 健康择优 · 故障转移 · 磁盘缓存</p>

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>引擎</h3>
          <span className={`pill ${running ? "ok" : "err"}`}>{running ? "运行中" : "未运行"}</span>
          {status && <span className="mono" style={{ opacity: 0.6 }}>127.0.0.1:{status.port}</span>}
          <span className="spacer" />
          {running ? (
            <button className="danger" onClick={() => void act(engineStop)}>停止</button>
          ) : (
            <button className="primary" onClick={() => void act(engineStart)}>启动</button>
          )}
        </div>
      </div>

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>工具接管</h3>
          <span className="spacer" />
          <button className="primary" onClick={() => void act(engineTakeover)}>一键接管 pip/npm/go</button>
          <button onClick={() => void act(engineRestore)}>还原官方源</button>
        </div>
        <table style={{ marginTop: 10 }}>
          <thead><tr><th>工具</th><th>当前指向</th><th>状态</th></tr></thead>
          <tbody>
            {pointing.map((p) => (
              <tr key={p.tool}>
                <td style={{ fontWeight: 600 }}>{p.tool}</td>
                <td className="mono" style={{ maxWidth: 320, overflow: "hidden", textOverflow: "ellipsis" }}>{p.current}</td>
                <td><span className={`pill ${p.on_engine ? "ok" : "dim"}`}>{p.on_engine ? "走引擎" : "未接管"}</span></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {status && (
        <div className="card">
          <div className="row">
            <h3 style={{ margin: 0 }}>镜像源管理</h3>
            <span className="toast" style={{ minHeight: 0 }}>每个生态可多上游,引擎按健康择优 · 改动即时重载</span>
            {busy && <span className="pill dim">重载中…</span>}
          </div>
          {status.ecosystems.map((e) => (
            <div key={e.name} style={{ marginTop: 14, paddingTop: 12, borderTop: "0.5px solid var(--border)" }}>
              <div className="row">
                <strong>{e.name}</strong>
                <span className="mono" style={{ opacity: 0.6 }}>{e.prefix}</span>
                <span className="pill dim">{e.upstreams.length} 个上游</span>
              </div>
              <div style={{ marginTop: 8 }}>
                {e.upstreams.map((u) => (
                  <div key={u.name} className="row" style={{ padding: "4px 0" }}>
                    <span className={`pill ${u.up ? "ok" : "err"}`}>{u.up ? `${u.latency.toFixed(1)}s` : "✗"}</span>
                    <strong style={{ minWidth: 90 }}>{u.name}</strong>
                    <span className="mono" style={{ flex: 1, opacity: 0.7, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{u.base}</span>
                    <button
                      className="danger"
                      disabled={busy || e.upstreams.length <= 1}
                      title={e.upstreams.length <= 1 ? "至少保留一个镜像" : "移除此镜像"}
                      onClick={() => void mutate(() => mirrorRemoveUpstream(e.name, u.name))}
                    >删</button>
                  </div>
                ))}
              </div>
              <div className="row" style={{ marginTop: 8 }}>
                <input
                  placeholder="镜像名,如 ustc"
                  value={upDraft(e.name).name}
                  disabled={busy}
                  onChange={(ev) => setUpDraft(e.name, { name: ev.target.value })}
                  style={{ width: 120 }}
                />
                <input
                  placeholder="https://镜像地址/…"
                  value={upDraft(e.name).base}
                  disabled={busy}
                  onChange={(ev) => setUpDraft(e.name, { base: ev.target.value })}
                  onKeyDown={(ev) => ev.key === "Enter" && void submitUpstream(e.name)}
                  style={{ flex: 1 }}
                />
                <button className="primary" disabled={busy} onClick={() => void submitUpstream(e.name)}>新增镜像</button>
              </div>
            </div>
          ))}

          <div style={{ marginTop: 16, paddingTop: 12, borderTop: "0.5px solid var(--border)" }}>
            <div className="row"><strong>新增生态</strong><span className="toast" style={{ minHeight: 0 }}>如私有仓库;前缀留空则用 /生态名/</span></div>
            <div className="row" style={{ marginTop: 8, gap: 6 }}>
              <input placeholder="生态名,如 conda" value={newEco.name} disabled={busy}
                onChange={(e) => setNewEco((p) => ({ ...p, name: e.target.value }))} style={{ width: 120 }} />
              <input placeholder="路由前缀 /conda/" value={newEco.prefix} disabled={busy}
                onChange={(e) => setNewEco((p) => ({ ...p, prefix: e.target.value }))} style={{ width: 120 }} />
              <input placeholder="首个镜像名" value={newEco.upName} disabled={busy}
                onChange={(e) => setNewEco((p) => ({ ...p, upName: e.target.value }))} style={{ width: 110 }} />
              <input placeholder="https://镜像地址/…" value={newEco.upBase} disabled={busy}
                onChange={(e) => setNewEco((p) => ({ ...p, upBase: e.target.value }))}
                onKeyDown={(e) => e.key === "Enter" && void submitEcosystem()} style={{ flex: 1 }} />
              <button className="primary" disabled={busy} onClick={() => void submitEcosystem()}>新增生态</button>
            </div>
          </div>
        </div>
      )}

      {status && (
        <div className="card">
          <div className="row">
            <h3 style={{ margin: 0 }}>缓存</h3>
            <span className="spacer" />
            <span className="mono">{formatBytes(status.cacheBytes)} / {status.cacheMaxMB} MB</span>
            <button onClick={() => void act(engineCacheClear)}>清空</button>
          </div>
          <div className="bar" style={{ marginTop: 10 }}>
            <div style={{ width: `${Math.min(cachePct, 100)}%` }} />
          </div>
        </div>
      )}

      <div className="card">
        <h3>分流诊断(实时)</h3>
        {events.length === 0 ? (
          <p className="toast">暂无流量。让 pip/npm 等工具通过引擎拉取后,这里会显示每次请求走哪个上游、是否命中缓存。</p>
        ) : (
          <table>
            <thead>
              <tr><th>时间</th><th>生态</th><th>路径</th><th>上游</th><th>缓存</th><th>状态</th><th>耗时</th></tr>
            </thead>
            <tbody>
              {events.map((e, i) => (
                <tr key={i}>
                  <td className="mono">{e.time}</td>
                  <td>{e.eco}</td>
                  <td className="mono" style={{ maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{e.path}</td>
                  <td>{e.upstream}</td>
                  <td>{e.cacheHit ? <span className="pill ok">HIT</span> : <span className="pill dim">MISS</span>}</td>
                  <td>{e.status}</td>
                  <td className="mono">{e.latencyMs}ms</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div className="toast">{toast}</div>
    </div>
  );
}
