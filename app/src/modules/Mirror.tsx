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
          <h3>各生态上游健康</h3>
          <table>
            <thead>
              <tr><th>生态</th><th>端点前缀</th><th>上游(延迟)</th></tr>
            </thead>
            <tbody>
              {status.ecosystems.map((e) => (
                <tr key={e.name}>
                  <td style={{ fontWeight: 600 }}>{e.name}</td>
                  <td className="mono">{e.prefix}</td>
                  <td>
                    {e.upstreams.map((u) => (
                      <span key={u.name} className={`pill ${u.up ? "ok" : "err"}`} style={{ marginRight: 6 }}>
                        {u.name} {u.up ? `${u.latency.toFixed(1)}s` : "✗"}
                      </span>
                    ))}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
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
