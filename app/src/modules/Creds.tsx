import { useEffect, useState } from "react";
import {
  credList,
  credGet,
  credSet,
  credDelete,
  ghAccounts,
  ghSwitch,
  type CredMeta,
  type GhAccount,
} from "../api";
import { getErrorMessage } from "../util";

const CATEGORIES = ["github", "cloudflare", "aws", "npm", "pypi", "other"];

export function Creds() {
  const [creds, setCreds] = useState<CredMeta[]>([]);
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [accounts, setAccounts] = useState<GhAccount[]>([]);
  const [switching, setSwitching] = useState("");
  const [draft, setDraft] = useState({ name: "", value: "", category: "github", note: "" });
  const [toast, setToast] = useState("");

  async function load(): Promise<void> {
    try {
      setCreds(await credList());
      setAccounts(await ghAccounts());
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  useEffect(() => {
    void load();
  }, []);

  async function doSwitch(user: string): Promise<void> {
    setSwitching(user);
    try {
      setToast(await ghSwitch(user));
      setAccounts(await ghAccounts()); // 重新读,确认活跃账号真的变了
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    } finally {
      setSwitching("");
    }
  }

  async function reveal(name: string): Promise<void> {
    try {
      const value = await credGet(name);
      setRevealed((prev) => ({ ...prev, [name]: value }));
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function inject(name: string): Promise<void> {
    try {
      const value = await credGet(name);
      await navigator.clipboard.writeText(`export ${name}='${value}'`);
      setToast(`已复制 export ${name}=… 到剪贴板,终端粘贴即注入`);
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function add(): Promise<void> {
    if (!draft.name.trim() || !draft.value.trim()) return;
    try {
      await credSet(draft.name.trim(), draft.value, draft.category, draft.note);
      setDraft({ name: "", value: "", category: draft.category, note: "" });
      setToast("已存入 Keychain");
      await load();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  async function remove(name: string): Promise<void> {
    try {
      await credDelete(name);
      await load();
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }

  return (
    <div>
      <h1 className="page-title">凭据账号</h1>
      <p className="page-sub">切换终端登录态 + Keychain 加密保管 · 仓库零明文</p>

      <div className="card">
        <h3>GitHub 登录态(终端当前账号)</h3>
        {accounts.length === 0 && <p className="toast">未检测到 gh 登录账号。</p>}
        {accounts.map((a) => (
          <div
            key={a.user}
            className="row"
            style={{
              padding: "8px 12px",
              marginBottom: 6,
              borderRadius: 8,
              background: a.active ? "var(--sel)" : "transparent",
              border: "0.5px solid var(--border)",
            }}
          >
            <span style={{ fontWeight: 600 }}>{a.user}</span>
            {a.active ? (
              <span className="pill ok">当前登录</span>
            ) : (
              <span className="pill dim">未激活</span>
            )}
            <span className="spacer" />
            {!a.active && (
              <button className="primary" disabled={switching === a.user} onClick={() => void doSwitch(a.user)}>
                {switching === a.user ? "切换中…" : "切到此账号"}
              </button>
            )}
          </div>
        ))}
        <p className="toast" style={{ marginTop: 6 }}>切换会同时更新 gh 登录态与 git 全局提交身份,终端里 git/gh 立即生效。</p>
      </div>

      <div className="card">
        <h3>新增凭据(存入 Keychain)</h3>
        <div className="row">
          <input placeholder="名称,如 CLOUDFLARE_API_TOKEN" value={draft.name}
            onChange={(e) => setDraft({ ...draft, name: e.target.value })} style={{ width: 230 }} />
          <select value={draft.category} onChange={(e) => setDraft({ ...draft, category: e.target.value })}>
            {CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
          </select>
          <input placeholder="值" type="password" value={draft.value}
            onChange={(e) => setDraft({ ...draft, value: e.target.value })} style={{ flex: 1, minWidth: 160 }} />
          <input placeholder="备注" value={draft.note}
            onChange={(e) => setDraft({ ...draft, note: e.target.value })} style={{ width: 110 }} />
          <button className="primary" onClick={() => void add()}>存入</button>
        </div>
      </div>

      <div className="card">
        <h3>已保管({creds.length})</h3>
        <table>
          <thead><tr><th>名称</th><th>分类</th><th>备注</th><th>值</th><th></th></tr></thead>
          <tbody>
            {creds.map((c) => (
              <tr key={c.name}>
                <td style={{ fontWeight: 600 }}>{c.name}</td>
                <td><span className="pill dim">{c.category}</span></td>
                <td style={{ opacity: 0.6 }}>{c.note}</td>
                <td className="mono" style={{ maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis" }}>
                  {revealed[c.name] ?? <button onClick={() => void reveal(c.name)}>显示</button>}
                </td>
                <td style={{ whiteSpace: "nowrap" }}>
                  <button onClick={() => void inject(c.name)}>注入</button>{" "}
                  <button className="danger" onClick={() => void remove(c.name)}>删</button>
                </td>
              </tr>
            ))}
            {creds.length === 0 && <tr><td colSpan={5} className="toast">还没有保管任何凭据。</td></tr>}
          </tbody>
        </table>
      </div>

      <div className="toast">{toast}</div>
    </div>
  );
}
