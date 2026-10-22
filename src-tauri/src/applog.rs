//! 应用日志：把启动过程与关键失败原因写入 app_data_dir/xtshell.log。
//!
//! 之前 MCP 绑定失败之类的问题只 eprintln 到不存在的控制台（GUI 程序没有
//! 终端窗口），用户完全无感；这里统一落盘，并由「日志」菜单查看。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// 日志文件上界 512 KB，超了在启动时裁到最近 2000 行。
const MAX_BYTES: u64 = 512 * 1024;
const KEEP_LINES: usize = 2000;
const NEWLINE: char = '\n';

static LOG_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<PathBuf>> {
    LOG_PATH.get_or_init(|| Mutex::new(None))
}

/// 设定日志文件位置（启动时调用一次），并把过大的历史文件裁小。
pub fn init(dir: &Path) -> PathBuf {
    let path = dir.join("xtshell.log");
    if std::fs::metadata(&path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        trim_tail(&path, KEEP_LINES);
    }
    *slot().lock().unwrap() = Some(path.clone());
    path
}

/// 日志文件路径（未初始化时为空）。
pub fn path() -> Option<PathBuf> {
    slot().lock().unwrap().clone()
}

pub fn info(message: &str) {
    append("INFO", message);
}


pub fn error(message: &str) {
    append("ERROR", message);
}

fn append(level: &str, message: &str) {
    let Some(path) = path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let _ = writeln!(file, "{} [{}] {}", timestamp(), level, message);
}

/// 读取最近 `limit` 行，**最新的排在最前面**（供界面直接渲染）。
pub fn tail(limit: usize) -> Vec<String> {
    let Some(path) = path() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() > limit {
        lines = lines.split_off(lines.len() - limit);
    }
    lines.iter().rev().map(|s| s.to_string()).collect()
}

/// 清空日志内容（保留文件本身）。
pub fn clear() {
    if let Some(path) = path() {
        let _ = std::fs::write(&path, b"");
    }
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 只保留文件最后 `keep` 行，避免无限增长。
fn trim_tail(path: &Path, keep: usize) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= keep {
        return;
    }
    let mut kept = String::with_capacity(text.len());
    for line in &lines[lines.len() - keep..] {
        kept.push_str(line);
        kept.push(NEWLINE);
    }
    let _ = std::fs::write(path, kept);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_read_newest_first() {
        let dir = std::env::temp_dir().join(format!("xtshell-applog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        init(&dir);
        info("first");
        info("second");
        let lines = tail(10);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("[INFO] second"), "最新在最前: {:?}", lines);
        assert!(lines[1].ends_with("[INFO] first"), "{:?}", lines);
        assert_eq!(tail(1).len(), 1);
        clear();
        assert!(tail(10).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trim_keeps_last_lines() {
        let dir = std::env::temp_dir().join(format!("xtshell-applog-trim-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.log");
        let mut body = String::new();
        for i in 0..50 {
            body.push_str(&format!("line{i}"));
            body.push(NEWLINE);
        }
        std::fs::write(&path, body).unwrap();
        trim_tail(&path, 5);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 5);
        assert!(text.starts_with("line45"), "{:?}", text);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
