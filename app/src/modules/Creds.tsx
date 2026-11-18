import { useEffect, useState } from "react";
import {
  credList,
  credGet,
  credSet,
  credDelete,
  ghAccounts,
  ghSwitch,
  type CredMeta,
} from "../api";
import { getErrorMessage } from "../util";

const CATEGORIES = ["github", "cloudflare", "aws", "npm", "pypi", "other"];

export function Creds() {
  const [creds, setCreds] = useState<CredMeta[]>([]);
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [gh, setGh] = useState("");
  const [draft, setDraft] = useState({ name: "", value: "", category: "github", note: "" });
  const [toast, setToast] = useState("");

  async function load(): Promise<void> {
    try {
      setCreds(await credList());
      setGh(await ghAccounts());
    } catch (error: unknown) {
      setToast(getErrorMessage(error));
    }
  }
  useEffect(() => {
    void load();
  }, []);

  async function reveal(name: string): Promise<void> {
    try {
      const v = await credGet(name);
      setRevealed((prev) => ({ ...prev, [name]: v }));
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

  // 从 gh auth status 文本里粗略抽账号名
  const ghUsers = Array.from(gh.matchAll(/account (\S+)/g)).map((m) => m[1]);

  return (
    <div>
      <h1 className="page-title">凭据账号</h1>
      <p className="page-sub">秘密只进 macOS Keychain(本地加密) · 仓库零明文</p>

      <div className="card">
        <h3>GitHub 多账号</h3>
        <div className="row">
          {ghUsers.length === 0 && <span className="toast">未检测到 gh 登录账号</span>}
          {ghUsers.map((u) => (
            <button key={u} onClick={() => void ghSwitch(u).then((r) => { setToast(r || `已切到 ${u}`); void load(); })}>
              切到 {u}
            </button>
          ))}
        </div>
        <pre className="mono" style={{ whiteSpace: "pre-wrap", opacity: 0.6, marginTop: 10, fontSize: 11 }}>{gh}</pre>
      </div>

      <div className="card">
        <h3>新增凭据</h3>
        <div className="row">
          <input placeholder="名称,如 CLOUDFLARE_API_TOKEN" value={draft.name}
            onChange={(e) => setDraft({ ...draft, name: e.target.value })} style={{ width: 230 }} />
          <select value={draft.category} onChange={(e) => setDraft({ ...draft, category: e.target.value })}>
            {CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
          </select>
          <input placeholder="值(token/key/password)" type="password" value={draft.value}
            onChange={(e) => setDraft({ ...draft, value: e.target.value })} style={{ flex: 1, minWidth: 180 }} />
          <input placeholder="备注" value={draft.note}
            onChange={(e) => setDraft({ ...draft, note: e.target.value })} style={{ width: 120 }} />
          <button className="primary" onClick={() => void add()}>存入</button>
        </div>
      </div>

      <div className="card">
        <h3>已保管</h3>
        <table>
          <thead><tr><th>名称</th><th>分类</th><th>备注</th><th>值</th><th></th></tr></thead>
          <tbody>
            {creds.map((c) => (
              <tr key={c.name}>
                <td style={{ fontWeight: 600 }}>{c.name}</td>
                <td><span className="pill dim">{c.category}</span></td>
                <td style={{ opacity: 0.6 }}>{c.note}</td>
                <td className="mono">
                  {revealed[c.name] ? revealed[c.name] : <button onClick={() => void reveal(c.name)}>显示</button>}
                </td>
                <td><button className="danger" onClick={() => void remove(c.name)}>删</button></td>
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
