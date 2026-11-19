// 公共助手:home 路径、执行外部命令。
use std::process::Command;

pub fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
}

/// 给当前进程注入完整 PATH。GUI app 默认 PATH 不含 Homebrew / mise 的 bin,
/// 导致 gh/npm/wrangler 等 command-not-found;启动时调用一次,所有子进程继承。
pub fn ensure_path() {
    let h = home();
    let extra = [
        "/opt/homebrew/bin".to_string(),
        "/opt/homebrew/sbin".to_string(),
        format!("{h}/.local/share/mise/shims"),
        format!("{h}/.local/bin"),
        format!("{h}/.cargo/bin"),
        "/usr/local/bin".to_string(),
    ];
    let cur = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<String> = extra.into_iter().filter(|p| !cur.contains(p.as_str())).collect();
    if !cur.is_empty() {
        parts.push(cur);
    }
    // 启动早期、单线程时设置,安全
    unsafe {
        std::env::set_var("PATH", parts.join(":"));
    }
}

/// 执行命令,成功返回 stdout,失败返回 stderr 文本。
pub fn sh(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// 写文件,自动建父目录。
pub fn write_file(path: &str, content: &str) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, content).map_err(|e| e.to_string())
}
