// 模块一:镜像源管理。与 Go 引擎(本地路由代理)通过 HTTP 控制接口交互,并读写其配置。
use crate::common::{home, sh, write_file};
use std::fs;

fn cfg_path() -> String {
    format!("{}/.config/proxyhub/config.json", home())
}

fn engine_bin() -> String {
    format!("{}/Workspace/proxyhub/engine/proxyhub", home())
}

fn port() -> u16 {
    if let Ok(s) = fs::read_to_string(cfg_path()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(p) = v.get("port").and_then(|p| p.as_u64()) {
                return p as u16;
            }
        }
    }
    8899
}

fn api(path: &str) -> Result<String, String> {
    sh(
        "curl",
        &[
            "-s",
            "--max-time",
            "3",
            &format!("http://127.0.0.1:{}{}", port(), path),
        ],
    )
}

#[tauri::command]
pub fn engine_status() -> Result<String, String> {
    api("/_ph/status")
}

#[tauri::command]
pub fn engine_diagnose() -> Result<String, String> {
    api("/_ph/diagnose")
}

#[tauri::command]
pub fn engine_cache_clear() -> Result<String, String> {
    api("/_ph/cache/clear")
}

#[tauri::command]
pub fn engine_running() -> bool {
    api("/_ph/status").is_ok()
}

#[tauri::command]
pub fn engine_start() -> Result<String, String> {
    let bin = engine_bin();
    if !std::path::Path::new(&bin).exists() {
        return Err(format!("引擎二进制不存在: {bin}(请先在 engine/ 下 go build)"));
    }
    std::process::Command::new(&bin)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok("引擎已启动".into())
}

#[tauri::command]
pub fn engine_stop() -> Result<String, String> {
    let _ = sh("pkill", &["-f", "proxyhub/engine/proxyhub"]);
    Ok("引擎已停止".into())
}

#[tauri::command]
pub fn mirror_config_get() -> Result<String, String> {
    fs::read_to_string(cfg_path()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mirror_config_save(json: String) -> Result<(), String> {
    // 校验是合法 JSON 再写
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| format!("非法 JSON: {e}"))?;
    write_file(&cfg_path(), &json)
}
