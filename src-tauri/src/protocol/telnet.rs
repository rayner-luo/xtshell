use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;

use crate::error::{AppError, AppResult};
use crate::session::manager::strip_ansi;
use crate::protocol::{ClosedSignal, OutputSink, TerminalStream};

const IAC: u8 = 255;
const SB: u8 = 250;
const SE: u8 = 240;

/// Telnet 带内命令过滤器：跨 feed 维护解析状态，避免 IAC 序列被读块边界切断后泄漏到终端。
pub struct TelnetParser {
    state: ParserState,
}

enum ParserState {
    Data,
    SawIac,
    Negotiation(#[allow(dead_code)] u8),
    Subnegotiation,
    SawIacInSub,
}

impl Default for TelnetParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TelnetParser {
    pub fn new() -> Self {
        TelnetParser { state: ParserState::Data }
    }

    pub fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for &b in data {
            match std::mem::replace(&mut self.state, ParserState::Data) {
                ParserState::Data => {
                    if b == IAC {
                        self.state = ParserState::SawIac;
                    } else {
                        out.push(b);
                    }
                }
                ParserState::SawIac => {
                    self.state = match b {
                        IAC => {
                            out.push(IAC);
                            ParserState::Data
                        }
                        SB => ParserState::Subnegotiation,
                        251..=254 => ParserState::Negotiation(b),
                        _ => ParserState::Data,
                    };
                }
                ParserState::Negotiation(_) => {
                    self.state = ParserState::Data;
                }
                ParserState::Subnegotiation => {
                    self.state = if b == IAC {
                        ParserState::SawIacInSub
                    } else {
                        ParserState::Subnegotiation
                    };
                }
                ParserState::SawIacInSub => {
                    self.state = match b {
                        SE => ParserState::Data,
                        _ => ParserState::Subnegotiation,
                    };
                }
            }
        }
        out
    }
}

pub struct TelnetStream {
    writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
}

impl TelnetStream {
    pub async fn connect(
        host: &str,
        port: u16,
        username: Option<String>,
        password: Option<String>,
        on_output: OutputSink,
        on_closed: ClosedSignal,
    ) -> AppResult<Self> {
        let stream = TcpStream::connect((host, port)).await
            .map_err(AppError::from)?;
        let (mut reader, writer) = stream.into_split();
        let writer = Arc::new(Mutex::new(Some(writer)));
        let mut parser = TelnetParser::new();
        let mut login = TelnetLoginState::new(username.as_deref(), password.as_deref());
        let task_writer = writer.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let clean = parser.feed(&buf[..n]);
                        if !clean.is_empty() {
                            on_output(&clean);
                            if let Some(prompt) = login.observe(&clean) {
                                let payload = match prompt {
                                    LoginPrompt::User => username
                                        .as_deref()
                                        .filter(|s| !s.is_empty())
                                        .map(|s| format!("{s}\r")),
                                    LoginPrompt::Password => password
                                        .as_deref()
                                        .filter(|s| !s.is_empty())
                                        .map(|s| format!("{s}\r")),
                                };
                                if let Some(bytes) = payload {
                                    let mut guard = task_writer.lock().await;
                                    if let Some(w) = guard.as_mut() {
                                        let _ = w.write_all(bytes.as_bytes()).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            on_closed();
        });
        Ok(TelnetStream { writer })
    }
}

#[async_trait]
impl TerminalStream for TelnetStream {
    async fn write(&mut self, data: &[u8]) -> AppResult<()> {
        let mut guard = self.writer.lock().await;
        let w = guard
            .as_mut()
            .ok_or_else(|| AppError::Connection("closed".into()))?;
        w.write_all(data).await.map_err(AppError::Io)?;
        Ok(())
    }

    async fn close(&mut self) -> AppResult<()> {
        let mut guard = self.writer.lock().await;
        if let Some(mut w) = guard.take() {
            let _ = w.shutdown().await;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginPrompt {
    User,
    Password,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginStep {
    WaitUser,
    WaitPassword,
    Done,
}

#[derive(Debug)]
struct TelnetLoginState {
    tail: Vec<u8>,
    step: LoginStep,
    has_username: bool,
    has_password: bool,
}

impl TelnetLoginState {
    fn new(username: Option<&str>, password: Option<&str>) -> Self {
        let has_username = username.map_or(false, |s| !s.is_empty());
        let has_password = password.map_or(false, |s| !s.is_empty());
        let step = if has_username {
            LoginStep::WaitUser
        } else if has_password {
            LoginStep::WaitPassword
        } else {
            LoginStep::Done
        };
        Self {
            tail: Vec::new(),
            step,
            has_username,
            has_password,
        }
    }

    fn observe(&mut self, data: &[u8]) -> Option<LoginPrompt> {
        if self.step == LoginStep::Done {
            return None;
        }
        self.tail.extend_from_slice(data);
        if self.tail.len() > 256 {
            let extra = self.tail.len() - 256;
            self.tail.drain(0..extra);
        }
        let clean = strip_ansi(&self.tail);
        let lower = String::from_utf8_lossy(&clean).to_lowercase();
        let line = lower
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim_end();
        let is_user_prompt = USER_PROMPTS.iter().any(|p| line.ends_with(p));
        let is_password_prompt = PASSWORD_PROMPTS.iter().any(|p| line.ends_with(p));

        match self.step {
            LoginStep::WaitUser => {
                if is_password_prompt {
                    self.step = LoginStep::Done;
                    return self.has_password.then_some(LoginPrompt::Password);
                }
                if is_user_prompt {
                    self.step = if self.has_password {
                        LoginStep::WaitPassword
                    } else {
                        LoginStep::Done
                    };
                    return self.has_username.then_some(LoginPrompt::User);
                }
                None
            }
            LoginStep::WaitPassword => {
                if is_password_prompt {
                    self.step = LoginStep::Done;
                    return self.has_password.then_some(LoginPrompt::Password);
                }
                None
            }
            LoginStep::Done => None,
        }
    }
}

const USER_PROMPTS: &[&str] = &["login:", "username:", "user name:", "user:"];
const PASSWORD_PROMPTS: &[&str] = &["password:", "passwd:", "密码:", "密码："];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_iac_negotiation() {
        let mut p = TelnetParser::new();
        let d = [255u8, 253, 24, b'h', b'i', 255, 250, 1, 1, 255, 240];
        assert_eq!(p.feed(&d), b"hi");
    }

    #[test]
    fn passthrough_plain_data() {
        let mut p = TelnetParser::new();
        assert_eq!(p.feed(b"hello"), b"hello");
    }

    #[test]
    fn iac_255_escaped() {
        let mut p = TelnetParser::new();
        assert_eq!(p.feed(&[255u8, 255, b'a']), &[255u8, b'a']);
    }

    #[test]
    fn trailing_iac_splits_across_feeds() {
        let mut p = TelnetParser::new();
        let out1 = p.feed(&[b'x', 255, 253]);
        let out2 = p.feed(&[24, b'y']);
        assert_eq!(out1, b"x");
        assert_eq!(out2, b"y");
    }

    #[test]
    fn subnegotiation_splits_across_feeds() {
        let mut p = TelnetParser::new();
        let out1 = p.feed(&[255, 250, 1, 1]);
        let out2 = p.feed(&[255, 240, b'z']);
        assert_eq!(out1, b"");
        assert_eq!(out2, b"z");
    }

    #[test]
    fn iac_iac_split() {
        let mut p = TelnetParser::new();
        let out1 = p.feed(&[255]);
        let out2 = p.feed(&[255, b'q']);
        assert_eq!(out1, b"");
        assert_eq!(out2, &[255u8, b'q']);
    }

    #[test]
    fn iac_iac_escape_inside_subnegotiation() {
        let mut p = TelnetParser::new();
        let d = [255u8, 250, 255, 255, b'q', 255, 240, b'w'];
        assert_eq!(p.feed(&d), b"w");
    }

    #[test]
    fn login_prompt_matches_case_insensitive() {
        let mut state = TelnetLoginState::new(Some("root"), Some("pass"));
        assert_eq!(state.observe(b"Login:"), Some(LoginPrompt::User));
        assert_eq!(state.observe(b"Password:"), Some(LoginPrompt::Password));
    }
}
