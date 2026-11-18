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

/// gh 多账号状态(原样返回 gh auth status 文本)。
#[tauri::command]
pub fn gh_accounts() -> Result<String, String> {
    let out = std::process::Command::new("gh")
        .args(["auth", "status"])
        .output()
        .map_err(|e| e.to_string())?;
    // gh auth status 可能走 stderr
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(s)
}

/// 切换 gh 活动账号(终端登录态即时切换)。
#[tauri::command]
pub fn gh_switch(user: String) -> Result<String, String> {
    sh("gh", &["auth", "switch", "-u", &user])
}
