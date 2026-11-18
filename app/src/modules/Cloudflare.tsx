import { useEffect, useState } from "react";
import {
  cfZones,
  cfEmailRules,
  cfEmailAdd,
  cfEmailDelete,
  cfEmailDestinations,
  cfOpenConsole,
} from "../api";
import { getErrorMessage, tryParse } from "../util";

interface Zone {
  id: string;
  name: string;
  account?: { id: string; name: string };
}
interface CfError {
  code: number;
  message: string;
}
interface CfResp<T> {
  success: boolean;
  result?: T;
  errors?: CfError[];
}
interface EmailRule {
  tag?: string;
  name?: string;
  enabled?: boolean;
  matchers?: { type: string; field?: string; value?: string }[];
  actions?: { type: string; value?: string[] }[];
}
interface Destination {
  email: string;
  verified?: string | null;
}

const CONSOLES = [
  { id: "workers", label: "Workers & Pages" },
  { id: "email", label: "Email Routing" },
  { id: "d1", label: "D1" },
  { id: "r2", label: "R2" },
];

export function Cloudflare() {
  const [zone, setZone] = useState<Zone | null>(null);
  const [rules, setRules] = useState<EmailRule[]>([]);
  const [dests, setDests] = useState<Destination[]>([]);
  const [alias, setAlias] = useState("");
  const [dest, setDest] = useState("");
  const [err, setErr] = useState("");
  const [toast, setToast] = useState("");

  async function loadAll(): Promise<void> {
    setErr("");
    try {
      const z = tryParse<CfResp<Zone[]>>(await cfZones());
      if (!z?.success) {
        setErr(`无法读取域名:${z?.errors?.map((e) => e.message).join("; ") || "token 无效或缺权限"}`);
        return;
      }
      const target = z.result?.find((x) => x.name === "ysoseri.us") ?? z.result?.[0] ?? null;
      setZone(target);
      if (!target) return;
      // 已验证的转发目标
      if (target.account?.id) {
        const d = tryParse<CfResp<Destination[]>>(await cfEmailDestinations(target.account.id));
        if (d?.success) {
          setDests(d.result ?? []);
          const firstVerified = (d.result ?? []).find((x) => x.verified)?.email;
          if (firstVerified) setDest(firstVerified);
        }
      }
      // 转发规则
      const r = tryParse<CfResp<EmailRule[]>>(await cfEmailRules(target.id));
      if (!r?.success) {
        setErr(
          `读取转发规则失败:${r?.errors?.map((e) => `${e.code} ${e.message}`).join("; ")}。` +
            `当前 token 很可能缺少「Email Routing Rules」权限——请换一个带该权限的 token。`
        );
        setRules([]);
        return;
      }
      setRules(r.result ?? []);
    } catch (error: unknown) {
      setErr(getErrorMessage(error));
    }
  }
  useEffect(() => {
    void loadAll();
  }, []);

  async function addAlias(): Promise<void> {
    if (!zone || !alias.trim() || !dest.trim()) return;
    try {
      const r = tryParse<CfResp<EmailRule>>(await cfEmailAdd(zone.id, alias.trim(), dest.trim()));
      setToast(r?.success ? `已创建 ${alias} → ${dest}` : `失败:${r?.errors?.map((e) => e.message).join("; ")}`);
      setAlias("");
      await loadAll();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function removeRule(tag?: string): Promise<void> {
    if (!zone || !tag) return;
    try {
      await cfEmailDelete(zone.id, tag);
      setToast("已删除规则");
      await loadAll();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  return (
    <div>
      <div className="row">
        <div>
          <h1 className="page-title">Cloudflare</h1>
          <p className="page-sub" style={{ margin: 0 }}>
            {zone ? `域名 ${zone.name} · 邮箱转发管理` : "控制台 + 域名邮箱(Email Routing)"}
          </p>
        </div>
        <span className="spacer" />
        {CONSOLES.map((c) => (
          <button key={c.id} onClick={() => void cfOpenConsole(c.id).then(setToast)}>{c.label}</button>
        ))}
      </div>

      {err && (
        <div className="card" style={{ borderColor: "var(--err)", marginTop: 16 }}>
          <strong style={{ color: "var(--err)" }}>⚠ {err}</strong>
        </div>
      )}

      <div className="card" style={{ marginTop: 16 }}>
        <h3>邮箱转发规则</h3>
        <div className="row" style={{ marginBottom: 10 }}>
          <input placeholder="别名,如 gh@ysoseri.us" value={alias}
            onChange={(e) => setAlias(e.target.value)} style={{ width: 230 }} />
          <span style={{ color: "var(--text-dim)" }}>→ 转发到</span>
          <select value={dest} onChange={(e) => setDest(e.target.value)}>
            {dests.filter((d) => d.verified).map((d) => <option key={d.email} value={d.email}>{d.email}</option>)}
            {dests.filter((d) => d.verified).length === 0 && <option value="">(无已验证目标)</option>}
          </select>
          <button className="primary" onClick={() => void addAlias()}>新增转发</button>
        </div>
        <table>
          <thead><tr><th>别名 / 匹配</th><th>转发到</th><th>状态</th><th></th></tr></thead>
          <tbody>
            {rules.map((r) => (
              <tr key={r.tag}>
                <td className="mono">{r.matchers?.map((m) => m.value ?? m.type).join(", ") || "(catch-all)"}</td>
                <td className="mono">{r.actions?.flatMap((a) => a.value ?? []).join(", ")}</td>
                <td><span className={`pill ${r.enabled ? "ok" : "dim"}`}>{r.enabled ? "启用" : "停用"}</span></td>
                <td><button className="danger" onClick={() => void removeRule(r.tag)}>删</button></td>
              </tr>
            ))}
            {rules.length === 0 && <tr><td colSpan={4} className="toast">无规则(或 token 无权限读取,见上方提示)。</td></tr>}
          </tbody>
        </table>
      </div>

      <div className="card">
        <h3>已验证的转发目标</h3>
        <div className="row">
          {dests.map((d) => (
            <span key={d.email} className={`pill ${d.verified ? "ok" : "err"}`}>
              {d.email} {d.verified ? "✓" : "未验证"}
            </span>
          ))}
          {dests.length === 0 && <span className="toast">未读到目标地址。</span>}
        </div>
      </div>

      <div className="toast">{toast}</div>
    </div>
  );
}
