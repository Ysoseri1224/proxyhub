// 模块一:镜像源管理。与 Go 引擎(本地路由代理)通过 HTTP 控制接口交互,并读写其配置。
use crate::common::{home, sh, write_file};
use std::fs;
use tauri::Manager;

fn cfg_path() -> String {
    format!("{}/.config/proxyhub/config.json", home())
}

/// 引擎二进制候选:优先用打包进 app 的,其次用工作区构建产物。
fn engine_candidates(app: &tauri::AppHandle) -> Vec<String> {
    let mut v = vec![];
    if let Ok(p) = app
        .path()
        .resolve("resources/proxyhub-engine", tauri::path::BaseDirectory::Resource)
    {
        v.push(p.to_string_lossy().to_string());
    }
    v.push(format!("{}/Workspace/proxyhub/engine/proxyhub", home()));
    v
}

/// 若引擎未运行则启动(app 启动时调用)。
pub fn ensure_engine(app: &tauri::AppHandle) {
    if engine_running() {
        return;
    }
    for bin in engine_candidates(app) {
        if std::path::Path::new(&bin).exists() {
            let _ = std::process::Command::new(&bin).spawn();
            return;
        }
    }
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
pub fn engine_start(app: tauri::AppHandle) -> Result<String, String> {
    if engine_running() {
        return Ok("引擎已在运行".into());
    }
    for bin in engine_candidates(&app) {
        if std::path::Path::new(&bin).exists() {
            std::process::Command::new(&bin)
                .spawn()
                .map_err(|e| e.to_string())?;
            // 确认真的在端口上监听,避免端口冲突导致"假启动"
            for _ in 0..12 {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if engine_running() {
                    return Ok("引擎已启动".into());
                }
            }
            return Err(format!(
                "引擎进程已拉起但未在端口 {} 监听(可能端口被占用或引擎异常退出)",
                port()
            ));
        }
    }
    Err("未找到引擎二进制(resources/proxyhub-engine 或工作区 engine/proxyhub)".into())
}

#[tauri::command]
pub fn engine_stop() -> Result<String, String> {
    // 按端口精确定位引擎进程并结束(避免误杀同名/路径含 proxyhub 的进程)
    let p = format!("tcp:{}", port());
    if let Ok(pids) = sh("lsof", &["-ti", &p]) {
        for pid in pids.split_whitespace() {
            let _ = sh("kill", &[pid]);
        }
    }
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
