use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::protocol::TerminalStream;

use super::output_buffer::OutputBuffer;

pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionConfig {
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub private_key: Option<String>,
    pub private_key_passphrase: Option<String>,
    pub serial_port: Option<String>,
    pub baud_rate: Option<u32>,
    pub data_bits: Option<u8>,
    pub stop_bits: Option<u8>,
    pub parity: Option<String>,
    /// 由已保存档案启动时记录其 id（见 `ProfileStore::build_session_config`）；
    /// SSH 认证失败时前端据此弹出凭据重输框。手工新建的会话为 None。
    #[serde(default)]
    pub profile_id: Option<String>,
    /// SSH 本地端口转发列表（连接成功后启动监听）
    #[serde(default)]
    pub forwards: Vec<crate::profile::PortForward>,
}

impl SessionConfig {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ssh(
        host: &str,
        port: u16,
        user: &str,
        password: Option<String>,
        private_key: Option<String>,
    ) -> Self {
        SessionConfig {
            protocol: "ssh".into(),
            host: Some(host.into()),
            port: Some(port),
            username: Some(user.into()),
            password,
            private_key,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Connecting,
    Connected,
    Closed,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub protocol: String,
    pub target: String,
    pub state: SessionState,
}

pub struct Session {
    pub id: SessionId,
    pub config: SessionConfig,
    pub state: SessionState,
    pub buffer: OutputBuffer,
    pub stream: Option<Box<dyn TerminalStream>>,
    pub log_file: Option<std::sync::Mutex<std::fs::File>>,
    pub log_path: Option<String>,
    /// 会话日志的行模型（把输出整理成终端所见的样子）
    log_screen: LogScreen,
}

pub struct SessionManager {
    sessions: HashMap<SessionId, Session>,
    buf_cap: usize,
}

pub type SharedManager = Arc<Mutex<SessionManager>>;

/// 剥离 ANSI 转义序列与 C0 控制字符（保留 \n \r \t），用于生成可读日志。
pub fn strip_ansi(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        match b {
            0x1b => {
                if i + 1 >= data.len() {
                    break;
                }
                match data[i + 1] {
                    b'[' => {
                        let mut j = i + 2;
                        while j < data.len() && (0x20..=0x3f).contains(&data[j]) {
                            j += 1;
                        }
                        if j < data.len() && (0x40..=0x7e).contains(&data[j]) {
                            i = j + 1;
                        } else {
                            break;
                        }
                    }
                    b']' => {
                        let mut end = None;
                        let mut j = i + 2;
                        while j < data.len() {
                            if data[j] == 0x07 {
                                end = Some(j + 1);
                                break;
                            }
                            if data[j] == 0x1b && j + 1 < data.len() && data[j + 1] == b'\\' {
                                end = Some(j + 2);
                                break;
                            }
                            j += 1;
                        }
                        match end {
                            Some(e) => i = e,
                            None => break,
                        }
                    }
                    b'P' | b'X' | b'^' | b'_' => {
                        let mut end = None;
                        let mut j = i + 2;
                        while j + 1 < data.len() {
                            if data[j] == 0x1b && data[j + 1] == b'\\' {
                                end = Some(j + 2);
                                break;
                            }
                            j += 1;
                        }
                        match end {
                            Some(e) => i = e,
                            None => break,
                        }
                    }
                    c if (0x20..=0x2f).contains(&c) => {
                        let mut j = i + 2;
                        while j < data.len() && (0x20..=0x2f).contains(&data[j]) {
                            j += 1;
                        }
                        if j < data.len() {
                            i = j + 1;
                        } else {
                            break;
                        }
                    }
                    _ => i += 2,
                }
            }
            0x07 | 0x08 => i += 1,
            c if c < 0x20 && c != b'\n' && c != b'\r' && c != b'\t' => i += 1,
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

/// 分页提示在读取边界上被切断时，最多需要滞留的字节数。
/// 覆盖 ` --More--` 到 `  ---- More ----`（含两侧填充）的规模。
const PAGER_HOLDBACK_MAX: usize = 32;

/// 片段是否可能是分页提示的前缀（`-{2,}` 后面还没到齐的 `More`）。
fn is_pager_prefix(s: &str) -> bool {
    let t = s.trim_start_matches([' ', '\t']);
    let dashes = t.len() - t.trim_start_matches('-').len();
    if dashes < 2 {
        return false;
    }
    let rest = t[dashes..].trim_start_matches([' ', '\t']);
    // `--`、`--More-` 这类尚未写完的提示；后置短横线允许未到齐
    "more--".starts_with(&rest.to_ascii_lowercase())
}

/// 缓冲区末尾若可能是被切断的分页提示，返回需要滞留（暂不落盘）的字节数。
fn pager_partial_tail(buf: &[u8]) -> usize {
    let max = buf.len().min(PAGER_HOLDBACK_MAX);
    for take in (2..=max).rev() {
        if let Ok(s) = std::str::from_utf8(&buf[buf.len() - take..]) {
            if is_pager_prefix(s) {
                return take;
            }
        }
    }
    0
}

/// `-{2,}` 之后若接一个完整分页提示，返回其结束位置。
fn pager_end(data: &[u8], after_dashes: usize) -> Option<usize> {
    let mut j = after_dashes;
    while j < data.len() && (data[j] == b' ' || data[j] == b'\t') {
        j += 1;
    }
    for expect in *b"more" {
        if j >= data.len() || data[j].to_ascii_lowercase() != expect {
            return None;
        }
        j += 1;
    }
    let mut k = j;
    while k < data.len() && (data[k] == b' ' || data[k] == b'\t') {
        k += 1;
    }
    let dashes_start = k;
    while k < data.len() && data[k] == b'-' {
        k += 1;
    }
    if k - dashes_start < 2 {
        // 允许「短横线 + more」这种没有后置短横线的形态，但要求其后就是行尾，
        // 免得把普通内容里出现的 `xxx-more` 误删
        if !(k >= data.len() || data[k] == b'\r' || data[k] == b'\n') {
            return None;
        }
        return Some(k);
    }
    // 吞掉提示后的填充空白（设备用它填满整行）
    while k < data.len() && (data[k] == b' ' || data[k] == b'\t') {
        k += 1;
    }
    Some(k)
}

/// 去掉输入流里的分页提示（` --More--`、`  ---- More ----` 等），
/// 连同提示前后的填充空白一起丢弃。
///
/// 设备把提示写进输出流，按空格续屏后用 `\r` 覆盖它；日志不做屏幕模拟，
/// 这些提示就成了脏数据，因此在落盘前过滤。
fn strip_pager(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'-' {
            let mut j = i;
            while j < data.len() && data[j] == b'-' {
                j += 1;
            }
            if j - i >= 2 {
                if let Some(end) = pager_end(data, j) {
                    // 提示前的填充空白也一并丢弃
                    while matches!(out.last(), Some(b) if *b == b' ' || *b == b'\t') {
                        out.pop();
                    }
                    i = end;
                    continue;
                }
            }
            out.extend_from_slice(&data[i..j]);
            i = j;
            continue;
        }
        out.push(data[i]);
        i += 1;
    }
    out
}

/// 从 `i` 开始的转义序列在缓冲区里是否已完整（不完整则需滞留等下一批字节）。
fn escape_complete(data: &[u8], i: usize) -> bool {
    let mut j = i + 1;
    if j >= data.len() {
        return false;
    }
    match data[j] {
        b'[' => {
            j += 1;
            while j < data.len() && (0x20..=0x3f).contains(&data[j]) {
                j += 1;
            }
            j < data.len()
        }
        b']' | b'P' | b'X' | b'^' | b'_' => {
            j += 1;
            while j < data.len() {
                if data[j] == 0x07 {
                    return true;
                }
                if data[j] == 0x1b {
                    return j + 1 < data.len();
                }
                j += 1;
            }
            false
        }
        _ => true,
    }
}

/// 取 CSI 参数串里第 `idx` 个数值（缺失或非法按 0 处理）。
fn csi_param(params: &[u8], idx: usize) -> usize {
    let s = String::from_utf8_lossy(params);
    let s = s.trim_start_matches(['?', '<', '=', '>']);
    s.split(';').nth(idx).unwrap_or("").trim().parse::<usize>().unwrap_or(0)
}

/// 会话日志的行模型：把输出整理成「终端屏幕上看到的样子」再落盘。
///
/// 直接按字节落盘会跑出两类脏数据（实测反馈）：
///   - readline 打印命令输出前会发一个裸 CR（CR LF + 关闭括号粘贴 + CR + 输出），
///     终端里只是回到行首，编辑器却把孤立 CR 当换行 → 日志凭空多出空行；
///   - 重画提示符同样是裸 CR + 相同内容，终端里互相覆盖只看见一份，
///     按字节落盘就成了 `root@…:~# root@…:~# ` 这样的重复。
///
/// 所以这里实现一个够用的行模型：CR 回行首（覆盖写）、LF 换行、BS 退格、
/// TAB 按 8 列对齐；CSI 只解释影响行内容的擦除与光标移动。
/// 被覆盖掉的旧内容不保留 —— 与终端所见一致。
struct LogScreen {
    /// 已收下但尚未定稿的输入：转义序列或分页提示可能被读取边界切断
    pending: Vec<u8>,
    /// 当前行内容
    line: Vec<u8>,
    /// 当前写入列
    col: usize,
}

impl LogScreen {
    fn new() -> Self {
        LogScreen { pending: Vec::new(), line: Vec::new(), col: 0 }
    }

    /// 收下一批输出，返回可以立即落盘的字节（已定稿的整行）。
    fn push(&mut self, data: &[u8]) -> Vec<u8> {
        self.pending.extend_from_slice(data);
        let hold = self.holdback();
        let upto = self.pending.len() - hold;
        if upto == 0 {
            return Vec::new();
        }
        let chunk: Vec<u8> = self.pending.drain(..upto).collect();
        let chunk = strip_pager(&chunk);
        let mut out = Vec::new();
        self.feed(&chunk, &mut out);
        out
    }

    /// 收尾：滞留输入与未定稿的行也一并吐出（停止日志 / 会话结束时调用）。
    fn finish(&mut self) -> Vec<u8> {
        let chunk = strip_pager(&std::mem::take(&mut self.pending));
        let mut out = Vec::new();
        self.feed(&chunk, &mut out);
        if !self.line.is_empty() {
            self.emit_line(&mut out);
        }
        out
    }

    /// 需要滞留、暂不处理的尾部长度：不完整的转义序列，或疑似被切断的分页提示。
    fn holdback(&self) -> usize {
        if let Some(i) = self.pending.iter().rposition(|b| *b == 0x1b) {
            if !escape_complete(&self.pending, i) {
                return self.pending.len() - i;
            }
        }
        pager_partial_tail(&self.pending)
    }

    fn feed(&mut self, data: &[u8], out: &mut Vec<u8>) {
        let mut i = 0;
        while i < data.len() {
            let b = data[i];
            match b {
                b'\r' => {
                    self.col = 0;
                    i += 1;
                }
                // VT/FF 在文本终端里同样是换行
                b'\n' | 0x0b | 0x0c => {
                    self.emit_line(out);
                    i += 1;
                }
                0x08 => {
                    self.col = self.col.saturating_sub(1);
                    i += 1;
                }
                b'\t' => {
                    let stop = (self.col / 8 + 1) * 8;
                    while self.col < stop {
                        self.put(b' ');
                    }
                    i += 1;
                }
                0x1b => i = self.escape(data, i),
                0x07 => i += 1,
                c if c < 0x20 => i += 1,
                c => {
                    self.put(c);
                    i += 1;
                }
            }
        }
    }

    /// 在 `col` 处写入一个字节：覆盖原内容，必要时用空格补齐。
    fn put(&mut self, b: u8) {
        if self.col < self.line.len() {
            self.line[self.col] = b;
        } else {
            while self.line.len() < self.col {
                self.line.push(b' ');
            }
            self.line.push(b);
        }
        self.col += 1;
    }

    /// 定稿当前行。空行照样保留 —— 设备输出里的空行在终端上也是空行。
    fn emit_line(&mut self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.line);
        out.extend_from_slice(b"\r\n");
        self.line.clear();
        self.col = 0;
    }

    /// 处理一个转义序列，返回其后的位置。
    fn escape(&mut self, data: &[u8], i: usize) -> usize {
        let mut j = i + 1;
        if j >= data.len() {
            return data.len();
        }
        match data[j] {
            b'[' => {
                j += 1;
                let start = j;
                while j < data.len() && (0x20..=0x3f).contains(&data[j]) {
                    j += 1;
                }
                if j >= data.len() {
                    return data.len();
                }
                let params: Vec<u8> = data[start..j].to_vec();
                let cmd = data[j];
                j += 1;
                let n = csi_param(&params, 0);
                match cmd {
                    // EL：擦除行内容（光标不动）
                    b'K' => match n {
                        0 => {
                            let cut = self.col.min(self.line.len());
                            self.line.truncate(cut);
                        }
                        1 => {
                            let cut = self.col.min(self.line.len());
                            self.line[..cut].fill(b' ');
                        }
                        _ => {
                            self.line.clear();
                            self.col = 0;
                        }
                    },
                    // 光标移动：只影响当前行的写入列
                    b'C' => self.col += n.max(1),
                    b'D' => self.col = self.col.saturating_sub(n.max(1)),
                    b'G' => self.col = n.saturating_sub(1),
                    b'H' | b'f' => self.col = csi_param(&params, 1).saturating_sub(1),
                    b'J' => {
                        if n >= 2 {
                            self.line.clear();
                            self.col = 0;
                        }
                    }
                    _ => {}
                }
                j
            }
            // OSC / DCS 等：跳过直到 BEL 或 ST
            b']' | b'P' | b'X' | b'^' | b'_' => {
                j += 1;
                while j < data.len() {
                    if data[j] == 0x07 {
                        return j + 1;
                    }
                    if data[j] == 0x1b {
                        return (j + 2).min(data.len());
                    }
                    j += 1;
                }
                data.len()
            }
            _ => j + 1,
        }
    }
}

impl SessionManager {
    pub fn new(buf_cap: usize) -> Self {
        SessionManager { sessions: HashMap::new(), buf_cap }
    }

    pub fn create(&mut self, config: SessionConfig) -> SessionId {
        let id = Uuid::new_v4().to_string();
        let session = Session {
            id: id.clone(),
            config,
            state: SessionState::Connecting,
            buffer: OutputBuffer::new(self.buf_cap),
            stream: None,
            log_file: None,
            log_path: None,
            log_screen: LogScreen::new(),
        };
        self.sessions.insert(id.clone(), session);
        id
    }

    pub fn attach_stream(&mut self, id: &str, stream: Box<dyn TerminalStream>) -> AppResult<()> {
        let s = self.get_mut(id)?;
        s.stream = Some(stream);
        s.state = SessionState::Connected;
        Ok(())
    }

    pub async fn send_input(&mut self, id: &str, data: &[u8]) -> AppResult<()> {
        let s = self.get_mut(id)?;
        match s.stream.as_mut() {
            Some(stream) => stream.write(data).await,
            None => Err(AppError::Connection("not connected".into())),
        }
    }

    pub async fn resize(&mut self, id: &str, cols: u32, rows: u32) -> AppResult<()> {
        let s = self.get_mut(id)?;
        if let Some(stream) = s.stream.as_mut() {
            stream.resize(cols, rows).await?;
        }
        Ok(())
    }

    pub fn inject_output_no_sink(&mut self, id: &str, data: &[u8]) -> AppResult<()> {
        let s = self.get_mut(id)?;
        s.buffer.append(data);
        let Session { log_file, log_screen, .. } = s;
        if let Some(f) = log_file.as_ref() {
            let out = log_screen.push(data);
            if !out.is_empty() {
                use std::io::Write;
                if let Ok(mut file) = f.lock() {
                    let _ = file.write_all(&out);
                    let _ = file.flush();
                }
            }
        }
        Ok(())
    }

    /// 开始把会话输出记录到 `path`（文件名由前端指定，缺失的父目录自动创建）。
    /// 返回实际使用的路径。
    pub fn start_log(&mut self, id: &str, path: &str) -> AppResult<String> {
        let s = self.get_mut(id)?;
        let full_path = std::path::Path::new(path);
        if let Some(parent) = full_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(AppError::Io)?;
            }
        }
        let file = std::fs::File::create(full_path).map_err(AppError::Io)?;
        s.log_file = Some(std::sync::Mutex::new(file));
        s.log_screen = LogScreen::new();
        s.log_path = Some(path.to_string());
        Ok(path.to_string())
    }

    pub fn stop_log(&mut self, id: &str) -> AppResult<String> {
        let s = self.get_mut(id)?;
        {
            // 收尾：滞留输入与未定稿的行也落盘，避免丢内容
            let Session { log_file, log_screen, .. } = s;
            let tail = log_screen.finish();
            if let Some(f) = log_file.as_ref() {
                if !tail.is_empty() {
                    use std::io::Write;
                    if let Ok(mut file) = f.lock() {
                        let _ = file.write_all(&tail);
                        let _ = file.flush();
                    }
                }
            }
        }
        s.log_file = None;
        s.log_path.take().ok_or_else(|| AppError::Connection("session is not logging".into()))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn log_path(&self, id: &str) -> Option<String> {
        self.get(id).ok().and_then(|s| s.log_path.clone())
    }
    pub fn get_output(&self, id: &str, offset: u64) -> AppResult<(Vec<u8>, u64)> {
        Ok(self.get(id)?.buffer.read_since(offset))
    }

    pub fn tail_output(&self, id: &str, n: usize) -> AppResult<Vec<u8>> {
        Ok(self.get(id)?.buffer.tail(n))
    }

    /// 移出会话并返回其终端流（同步操作，不跨 await）。
    /// 调用方须先释放 manager 锁，再 await `stream.close()`，
    /// 避免全局锁被慢速网络关闭阻塞。
    pub fn take_for_close(&mut self, id: &str) -> AppResult<Option<Box<dyn TerminalStream>>> {
        let mut s = self
            .sessions
            .remove(id)
            .ok_or_else(|| AppError::SessionNotFound(id.into()))?;
        Ok(s.stream.take())
    }

    pub fn mark_closed(&mut self, id: &str) -> AppResult<()> {
        if let Some(s) = self.sessions.get_mut(id) {
            s.state = SessionState::Closed;
        }
        Ok(())
    }

    pub fn take_closed(&mut self, id: &str) -> AppResult<SessionConfig> {
        let s = self.sessions.remove(id).ok_or_else(|| AppError::SessionNotFound(id.into()))?;
        if s.state != SessionState::Closed {
            self.sessions.insert(id.to_string(), s);
            return Err(AppError::Connection("session is still active".into()));
        }
        Ok(s.config)
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        self.sessions
            .values()
            .map(|s| SessionInfo {
                id: s.id.clone(),
                protocol: s.config.protocol.clone(),
                target: s
                    .config
                    .host
                    .clone()
                    .or_else(|| s.config.serial_port.clone())
                    .unwrap_or_default(),
                state: s.state,
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> AppResult<&Session> {
        self.sessions.get(id).ok_or_else(|| AppError::SessionNotFound(id.into()))
    }

    fn get_mut(&mut self, id: &str) -> AppResult<&mut Session> {
        self.sessions.get_mut(id).ok_or_else(|| AppError::SessionNotFound(id.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppResult;
    use crate::protocol::TerminalStream;

    struct MockStream {
        written: Arc<std::sync::Mutex<Vec<u8>>>,
        closed: Arc<std::sync::Mutex<bool>>,
    }

    #[async_trait::async_trait]
    impl TerminalStream for MockStream {
        async fn write(&mut self, d: &[u8]) -> AppResult<()> {
            self.written.lock().unwrap().extend_from_slice(d);
            Ok(())
        }
        async fn close(&mut self) -> AppResult<()> {
            *self.closed.lock().unwrap() = true;
            Ok(())
        }
    }

    #[tokio::test]
    async fn create_and_close_session() {
        let mut mgr = SessionManager::new(64);
        let id = mgr.create(SessionConfig::ssh("h", 22, "u", Some("p".into()), None));
        assert_eq!(mgr.list().len(), 1);
        if let Some(mut stream) = mgr.take_for_close(&id).unwrap() {
            stream.close().await.unwrap();
        }
        assert!(mgr.list().is_empty());
    }

    #[tokio::test]
    async fn session_log_lifecycle() {
        let dir = std::env::temp_dir().join(format!("xtshell-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut mgr = SessionManager::new(64);
        let id = mgr.create(SessionConfig::ssh("1.2.3.4", 22, "u", None, None));
        mgr.inject_output_no_sink(&id, b"not logged").unwrap();
        assert!(mgr.log_path(&id).is_none());

        let log_file = dir.join("20260101_000000.txt");
        let path = mgr.start_log(&id, log_file.to_str().unwrap()).unwrap();
        assert!(path.ends_with("20260101_000000.txt"));
        mgr.inject_output_no_sink(&id, b"hello log\r\n").unwrap();

        assert_eq!(mgr.log_path(&id).as_deref(), Some(path.as_str()));
        // 整行即落盘
        let content = std::fs::read(&path).unwrap();
        assert_eq!(content, b"hello log\r\n");

        // 没有换行结尾的残缺行在停止时收尾落盘，不丢内容
        mgr.inject_output_no_sink(&id, b"tail without newline").unwrap();
        let stopped = mgr.stop_log(&id).unwrap();
        assert_eq!(stopped, path);
        let content = std::fs::read(&path).unwrap();
        assert_eq!(content, b"hello log\r\ntail without newline\r\n");
        assert!(mgr.stop_log(&id).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_screen_matches_terminal_for_readline_output() {
        // 本机 PTY 复现的真实字节：输入 `echo hi` 回车后是
        // CR LF + `\x1b[?2004l` + CR + `hi` + CR LF + `\x1b[?2004h` + 新提示符。
        // 终端上是三行；那个裸 CR 不该在日志里变成空行。
        let mut s = LogScreen::new();
        let mut out = s.push(b"root@host:~# echo hi");
        out.extend(s.push(b"\r\n\x1b[?2004l\rhi\r\n\x1b[?2004hroot@host:~# "));
        out.extend(s.finish());
        assert_eq!(out, b"root@host:~# echo hi\r\nhi\r\nroot@host:~# \r\n");
    }

    #[test]
    fn log_screen_collapses_redrawn_prompt() {
        // readline 重画提示符：裸 CR + 相同内容。终端只看见一份，
        // 日志也必须只有一份（旧实现会写成 `root@…:~# root@…:~# `）。
        let mut s = LogScreen::new();
        let mut out = s.push(b"root@host:~# \rroot@host:~# \rroot@host:~# ");
        out.extend(s.finish());
        assert_eq!(out, b"root@host:~# \r\n");
    }

    #[test]
    fn log_screen_keeps_real_blank_lines_only() {
        // `\r\r\n` 是终端里的一次换行（编辑器会显示成两个）；`\r\n\r\n` 才是真的空行。
        let mut s = LogScreen::new();
        let mut out = s.push(b"a\r\r\n\r\nb\r\n");
        out.extend(s.finish());
        assert_eq!(out, b"a\r\n\r\nb\r\n");
    }

    #[test]
    fn log_screen_applies_erase_and_redraw() {
        let mut s = LogScreen::new();
        let mut out = s.push(b"root@host:~# ls\x1b[K\rroot@host:~# ls -l");
        out.extend(s.finish());
        assert_eq!(out, b"root@host:~# ls -l\r\n");
    }

    #[test]
    fn log_screen_holds_split_escape_sequence() {
        // 转义序列被读取边界切断：先滞留，拼接后再处理
        let mut s = LogScreen::new();
        assert_eq!(s.push(b"abc\x1b["), b"");
        let mut out = s.push(b"Kdef");
        out.extend(s.finish());
        assert_eq!(out, b"abcdef\r\n");
    }

    #[test]
    fn log_screen_expands_tabs() {
        let mut s = LogScreen::new();
        let mut out = s.push(b"ab\tcd\r\n");
        out.extend(s.finish());
        assert_eq!(out, b"ab      cd\r\n");
    }

    #[tokio::test]
    async fn session_log_filters_pager_prompt() {
        // 交换机/路由器的分页提示不该进日志：锐捷 ` --More--`、华为 `  ---- More ----`
        let dir = std::env::temp_dir().join(format!("xtshell-log-pager-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut mgr = SessionManager::new(64);
        let id = mgr.create(SessionConfig::ssh("sw", 22, "admin", None, None));
        let log_file = dir.join("pager.txt");
        mgr.start_log(&id, log_file.to_str().unwrap()).unwrap();
        mgr.inject_output_no_sink(&id, b"!\r\n --More--           vlan 1\r\n").unwrap();
        mgr.inject_output_no_sink(&id, b"  ---- More ----      ip route\r\n").unwrap();
        mgr.stop_log(&id).unwrap();
        let text = std::fs::read_to_string(&log_file).unwrap();
        assert_eq!(text, "!\r\nvlan 1\r\nip route\r\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strip_pager_handles_split_boundaries() {
        // 提示被读取边界切断：前半段应被滞留，与后半段拼接后再过滤
        let a = b"!\r\n --Mo";
        assert_eq!(pager_partial_tail(a), 5);
        let mut buf = a.to_vec();
        buf.extend_from_slice(b"re--      vlan 1\r\n");
        let hold = pager_partial_tail(&buf);
        assert_eq!(hold, 0);
        assert_eq!(strip_pager(&buf), b"!\r\nvlan 1\r\n");
    }

    #[test]
    fn strip_pager_keeps_plain_content() {
        // 普通短横线分隔行与含 more 的正文都不该被误删
        let d = b"----------\r\nno more words\r\n";
        assert_eq!(strip_pager(d), d.to_vec());
    }

    #[test]
    fn strip_ansi_removes_csi_osc_and_keeps_text() {
        let d = b"\x1b[?2004h\x1b[Klsblk\n\x1b[?2004l\nroot@host:~# NAME   MAJ:MIN\nvda    254:0";
        assert_eq!(
            strip_ansi(d),
            b"lsblk\n\nroot@host:~# NAME   MAJ:MIN\nvda    254:0".to_vec()
        );
    }

    #[test]
    fn strip_ansi_removes_osc_title() {
        let d = b"\x1b]0;user@debian: ~\x07root@host:~# blkid\r\n";
        assert_eq!(strip_ansi(d), b"root@host:~# blkid\r\n".to_vec());
    }

    #[test]
    fn strip_ansi_keeps_colors_removed_and_text_intact() {
        let d = b"\x1b[01;32mroot\x1b[00m:\x1b[01;34m~\x1b[00m$ echo ok\n";
        assert_eq!(strip_ansi(d), b"root:~$ echo ok\n".to_vec());
    }

    #[tokio::test]
    async fn send_input_writes_to_stream() {
        let mut mgr = SessionManager::new(64);
        let id = mgr.create(SessionConfig::ssh("h", 22, "u", Some("p".into()), None));
        let written: Arc<std::sync::Mutex<Vec<u8>>> = Arc::new(std::sync::Mutex::new(vec![]));
        let closed: Arc<std::sync::Mutex<bool>> = Arc::new(std::sync::Mutex::new(false));
        mgr.attach_stream(&id, Box::new(MockStream { written: written.clone(), closed })).unwrap();
        mgr.send_input(&id, b"hi").await.unwrap();
        assert_eq!(mgr.get(&id).unwrap().state, SessionState::Connected);
        assert_eq!(&*written.lock().unwrap(), b"hi");
    }

    #[tokio::test]
    async fn inject_output_goes_to_buffer() {
        let mut mgr = SessionManager::new(64);
        let id = mgr.create(SessionConfig::ssh("h", 22, "u", None, None));
        mgr.inject_output_no_sink(&id, b"hello").unwrap();
        let (out, next) = mgr.get_output(&id, 0).unwrap();
        assert_eq!(out, b"hello");
        assert_eq!(next, 5);
    }

    #[tokio::test]
    async fn get_output_unknown_id_errors() {
        let mgr = SessionManager::new(64);
        assert!(mgr.get_output("nope", 0).is_err());
    }
}
