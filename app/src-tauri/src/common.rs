// 公共助手:home 路径、执行外部命令。
use std::process::Command;

pub fn home() -> String {
    std::env::var("HOME").unwrap_or_default()
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
