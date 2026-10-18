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
        if let Some(f) = s.log_file.as_ref() {
            use std::io::Write;
            if let Ok(mut file) = f.lock() {
                let _ = file.write_all(&strip_ansi(data));
                let _ = file.flush();
            }
        }
        Ok(())
    }

    pub fn start_log(&mut self, id: &str, dir: &str, timestamp: &str) -> AppResult<String> {
        let s = self.get_mut(id)?;
        let target = s
            .config
            .host
            .clone()
            .or_else(|| s.config.serial_port.clone())
            .unwrap_or_else(|| s.config.protocol.clone());
        let safe_target: String = target
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let file_name = format!("{timestamp}_{safe_target}.log");
        std::fs::create_dir_all(dir).map_err(AppError::Io)?;
        let full_path = std::path::Path::new(dir).join(&file_name);
        let file = std::fs::File::create(&full_path).map_err(AppError::Io)?;
        let path_str = full_path.to_string_lossy().to_string();
        s.log_file = Some(std::sync::Mutex::new(file));
        s.log_path = Some(path_str.clone());
        Ok(path_str)
    }

    pub fn stop_log(&mut self, id: &str) -> AppResult<String> {
        let s = self.get_mut(id)?;
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

        let path = mgr.start_log(&id, dir.to_str().unwrap(), "20260101_000000").unwrap();
        assert!(path.ends_with("20260101_000000_1.2.3.4.log"));
        mgr.inject_output_no_sink(&id, b"hello log").unwrap();

        assert_eq!(mgr.log_path(&id).as_deref(), Some(path.as_str()));
        let content = std::fs::read(&path).unwrap();
        assert_eq!(content, b"hello log");

        let stopped = mgr.stop_log(&id).unwrap();
        assert_eq!(stopped, path);
        assert!(mgr.stop_log(&id).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strip_ansi_removes_csi_osc_and_keeps_text() {
        let d = b"\x1b[?2004h\x1b[Klsblk\n\x1b[?2004l\nroot@ecs-0902:~# NAME   MAJ:MIN\nvda    254:0";
        assert_eq!(
            strip_ansi(d),
            b"lsblk\n\nroot@ecs-0902:~# NAME   MAJ:MIN\nvda    254:0".to_vec()
        );
    }

    #[test]
    fn strip_ansi_removes_osc_title() {
        let d = b"\x1b]0;rayner@debian: ~\x07root@ecs-0902:~# blkid\r\n";
        assert_eq!(strip_ansi(d), b"root@ecs-0902:~# blkid\r\n".to_vec());
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
