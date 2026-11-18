import { useState } from "react";
import {
  cfVerify,
  cfZones,
  cfEmailRules,
  cfEmailAdd,
  cfOpenConsole,
} from "../api";
import { getErrorMessage, tryParse } from "../util";

interface Zone {
  id: string;
  name: string;
  account?: { id: string; name: string };
}
interface ZonesResp {
  success: boolean;
  result?: Zone[];
}
interface EmailRule {
  name?: string;
  enabled?: boolean;
  matchers?: { type: string; field?: string; value?: string }[];
  actions?: { type: string; value?: string[] }[];
}
interface RulesResp {
  success: boolean;
  result?: EmailRule[];
}

const CONSOLES = [
  { id: "workers", label: "Workers & Pages" },
  { id: "email", label: "Email Routing" },
  { id: "d1", label: "D1" },
  { id: "r2", label: "R2" },
];

export function Cloudflare() {
  const [zones, setZones] = useState<Zone[]>([]);
  const [zoneId, setZoneId] = useState("");
  const [rules, setRules] = useState<EmailRule[]>([]);
  const [alias, setAlias] = useState("");
  const [dest, setDest] = useState("");
  const [toast, setToast] = useState("");

  async function loadZones(): Promise<void> {
    try {
      const v = tryParse<{ success: boolean }>(await cfVerify());
      if (!v?.success) {
        setToast("token 无效或未在凭据库存入 CLOUDFLARE_API_TOKEN");
        return;
      }
      const z = tryParse<ZonesResp>(await cfZones());
      setZones(z?.result ?? []);
      setToast(`token 有效,拉到 ${z?.result?.length ?? 0} 个域名`);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function loadRules(id: string): Promise<void> {
    setZoneId(id);
    try {
      const r = tryParse<RulesResp>(await cfEmailRules(id));
      setRules(r?.result ?? []);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function addAlias(): Promise<void> {
    if (!zoneId || !alias.trim() || !dest.trim()) return;
    try {
      const r = tryParse<{ success: boolean }>(await cfEmailAdd(zoneId, alias.trim(), dest.trim()));
      setToast(r?.success ? "别名已创建" : "创建失败(目标地址需先在 CF 验证)");
      setAlias("");
      await loadRules(zoneId);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  return (
    <div>
      <h1 className="page-title">Cloudflare</h1>
      <p className="page-sub">控制台快捷入口 + 域名邮箱(Email Routing)管理 · token 取自凭据库</p>

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>控制台快捷入口</h3>
          <span className="spacer" />
          {CONSOLES.map((c) => (
            <button key={c.id} onClick={() => void cfOpenConsole(c.id).then(setToast)}>{c.label}</button>
          ))}
        </div>
      </div>

      <div className="card">
        <div className="row">
          <h3 style={{ margin: 0 }}>域名</h3>
          <span className="spacer" />
          <button className="primary" onClick={() => void loadZones()}>校验 token 并加载</button>
        </div>
        <div className="row" style={{ marginTop: 10 }}>
          {zones.map((z) => (
            <button key={z.id} className={zoneId === z.id ? "primary" : ""} onClick={() => void loadRules(z.id)}>
              {z.name}
            </button>
          ))}
          {zones.length === 0 && <span className="toast">点右上加载;需先在"凭据账号"存入 CLOUDFLARE_API_TOKEN。</span>}
        </div>
      </div>

      {zoneId && (
        <div className="card">
          <h3>邮箱路由规则 — {zones.find((z) => z.id === zoneId)?.name}</h3>
          <div className="row">
            <input placeholder="别名,如 leif@ysoseri.us" value={alias} onChange={(e) => setAlias(e.target.value)} style={{ width: 230 }} />
            <span>→</span>
            <input placeholder="转发到(已验证地址)" value={dest} onChange={(e) => setDest(e.target.value)} style={{ flex: 1, minWidth: 180 }} />
            <button className="primary" onClick={() => void addAlias()}>新增转发</button>
          </div>
          <table style={{ marginTop: 12 }}>
            <thead><tr><th>别名 / 匹配</th><th>转发到</th><th>状态</th></tr></thead>
            <tbody>
              {rules.map((r, i) => (
                <tr key={i}>
                  <td className="mono">{r.matchers?.map((m) => m.value ?? m.type).join(", ") || "(catch-all)"}</td>
                  <td className="mono">{r.actions?.flatMap((a) => a.value ?? []).join(", ")}</td>
                  <td><span className={`pill ${r.enabled ? "ok" : "dim"}`}>{r.enabled ? "启用" : "停用"}</span></td>
                </tr>
              ))}
              {rules.length === 0 && <tr><td colSpan={3} className="toast">无规则。</td></tr>}
            </tbody>
          </table>
        </div>
      )}

      <div className="toast">{toast}</div>
    </div>
  );
}
