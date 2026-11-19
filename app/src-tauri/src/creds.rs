// 模块三:凭据与账号管理。秘密只进 macOS Keychain(本地、加密),索引只存元数据(无明文)。
use crate::common::{home, sh, write_file};
use serde::{Deserialize, Serialize};
use std::fs;

const SERVICE: &str = "proxyhub";

#[derive(Serialize, Deserialize, Clone)]
pub struct CredMeta {
    pub name: String,     // Keychain account 名,同时是显示名
    pub category: String, // github | cloudflare | aws | npm | pypi | other
    pub note: String,
}

fn index_path() -> String {
    format!("{}/.config/proxyhub/creds-index.json", home())
}

fn load_index() -> Vec<CredMeta> {
    fs::read_to_string(index_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_index(idx: &[CredMeta]) -> Result<(), String> {
    write_file(
        &index_path(),
        &serde_json::to_string_pretty(idx).map_err(|e| e.to_string())?,
    )
}

/// 列出已保管的凭据(仅元数据,不含明文)。
#[tauri::command]
pub fn cred_list() -> Result<Vec<CredMeta>, String> {
    Ok(load_index())
}

/// 读取某凭据的明文(从 Keychain,可能触发 Touch ID)。
#[tauri::command]
pub fn cred_get(name: String) -> Result<String, String> {
    sh(
        "security",
        &["find-generic-password", "-s", SERVICE, "-a", &name, "-w"],
    )
    .map(|s| s.trim_end().to_string())
}

/// 存/更新一个凭据:明文进 Keychain,元数据进索引。
#[tauri::command]
pub fn cred_set(name: String, value: String, category: String, note: String) -> Result<(), String> {
    sh(
        "security",
        &[
            "add-generic-password",
            "-U",
            "-s",
            SERVICE,
            "-a",
            &name,
            "-w",
            &value,
        ],
    )?;
    let mut idx = load_index();
    idx.retain(|c| c.name != name);
    idx.push(CredMeta {
        name,
        category,
        note,
    });
    save_index(&idx)
}

#[tauri::command]
pub fn cred_delete(name: String) -> Result<(), String> {
    let _ = sh(
        "security",
        &["delete-generic-password", "-s", SERVICE, "-a", &name],
    );
    let mut idx = load_index();
    idx.retain(|c| c.name != name);
    save_index(&idx)
}

#[derive(Serialize)]
pub struct GhAccount {
    pub user: String,
    pub active: bool,
}

/// gh 多账号:解析出每个账号及谁是当前活跃账号(终端登录态)。
#[tauri::command]
pub fn gh_accounts() -> Result<Vec<GhAccount>, String> {
    let out = std::process::Command::new("gh")
        .args(["auth", "status"])
        .output()
        .map_err(|e| e.to_string())?;
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));

    let mut accounts: Vec<GhAccount> = vec![];
    for line in text.lines() {
        if let Some(idx) = line.find("account ") {
            let rest = &line[idx + "account ".len()..];
            let user = rest.split_whitespace().next().unwrap_or("").to_string();
            if !user.is_empty() {
                accounts.push(GhAccount { user, active: false });
            }
        }
        if line.contains("Active account: true") {
            if let Some(last) = accounts.last_mut() {
                last.active = true;
            }
        }
    }
    Ok(accounts)
}

/// 切换 gh 活动账号(终端登录态即时切换),并同步 git 全局身份。
#[tauri::command]
pub fn gh_switch(user: String) -> Result<String, String> {
    sh("gh", &["auth", "switch", "-u", &user])?;
    // 同步 git 提交身份,避免切了 gh 却还用旧 user.name
    let _ = sh("git", &["config", "--global", "user.name", &user]);
    Ok(format!("已切换到 {user}"))
}

fn guess_cat(name: &str) -> String {
    let n = name.to_lowercase();
    for (kw, cat) in [
        ("github", "github"),
        ("gh_", "github"),
        ("cloudflare", "cloudflare"),
        ("cf_", "cloudflare"),
        ("aws", "aws"),
        ("npm", "npm"),
        ("pypi", "pypi"),
        ("openai", "ai"),
        ("anthropic", "ai"),
    ] {
        if n.contains(kw) {
            return cat.to_string();
        }
    }
    "other".to_string()
}

/// 把本地密码本(~/.secrets/密码本.md)里的所有凭据一次性导入 Keychain + 索引。
#[tauri::command]
pub fn import_password_book() -> Result<String, String> {
    let path = format!("{}/.secrets/密码本.md", home());
    let content = fs::read_to_string(&path).map_err(|e| format!("读密码本失败: {e}"))?;
    let mut idx = load_index();
    let mut count = 0;
    for line in content.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = t.trim_matches('|').split('|').map(|c| c.trim()).collect();
        if cols.len() < 3 {
            continue;
        }
        let name = cols[0].trim_matches('`').trim();
        let note = cols[1];
        let value = cols[2].trim_matches('`').trim();
        // 只收像 key=value 的行:名称像标识符、值非空且非分隔符
        if name.len() < 3 || value.is_empty() || value.contains("---") {
            continue;
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            continue;
        }
        if sh(
            "security",
            &["add-generic-password", "-U", "-s", SERVICE, "-a", name, "-w", value],
        )
        .is_ok()
        {
            idx.retain(|c| c.name != name);
            idx.push(CredMeta {
                name: name.to_string(),
                category: guess_cat(name),
                note: note.to_string(),
            });
            count += 1;
        }
    }
    save_index(&idx)?;
    Ok(format!("已从密码本导入 {count} 条到 Keychain"))
}

#[derive(Serialize)]
pub struct ToolLogin {
    pub tool: String,
    pub logged_in: bool,
    pub detail: String,
    pub login_cmd: String,
}

fn has(path: &str, needle: &str) -> bool {
    fs::read_to_string(path)
        .map(|s| s.contains(needle))
        .unwrap_or(false)
}

/// 探测各开发工具/环境的本地登录态(文件级检测,快且不触网)。
#[tauri::command]
pub fn tool_logins() -> Result<Vec<ToolLogin>, String> {
    let h = home();
    let mut out = vec![];

    // GitHub(gh)—读本地 keyring 状态
    let gh = gh_accounts().unwrap_or_default();
    let active = gh.iter().find(|a| a.active).map(|a| a.user.clone());
    out.push(ToolLogin {
        tool: "GitHub (gh)".into(),
        logged_in: active.is_some(),
        detail: active.unwrap_or_else(|| "未登录".into()),
        login_cmd: "gh auth login".into(),
    });

    let checks = [
        ("npm", format!("{h}/.npmrc"), "_authToken", "npm login"),
        ("Cloudflare (wrangler)", format!("{h}/Library/Preferences/.wrangler/config/default.toml"), "oauth_token", "wrangler login"),
        ("Docker", format!("{h}/.docker/config.json"), "auth", "docker login"),
        ("HuggingFace", format!("{h}/.cache/huggingface/token"), "", "hf auth login"),
        ("AWS", format!("{h}/.aws/credentials"), "", "aws configure"),
        ("Cargo (crates.io)", format!("{h}/.cargo/credentials.toml"), "", "cargo login"),
        ("PyPI (twine)", format!("{h}/.pypirc"), "", "twine upload"),
    ];
    for (tool, path, needle, cmd) in checks {
        let logged = if needle.is_empty() {
            std::path::Path::new(&path).exists()
        } else {
            has(&path, needle)
        };
        out.push(ToolLogin {
            tool: tool.into(),
            logged_in: logged,
            detail: if logged { "已登录".into() } else { "未登录".into() },
            login_cmd: cmd.into(),
        });
    }
    Ok(out)
}

/// 在系统终端里打开并运行某命令(用于交互式登录/切换,认证环节交给用户完成)。
#[tauri::command]
pub fn run_in_terminal(cmd: String) -> Result<String, String> {
    let escaped = cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("tell application \"Terminal\"\nactivate\ndo script \"{escaped}\"\nend tell");
    sh("osascript", &["-e", &script])?;
    Ok("已在终端打开".into())
}
