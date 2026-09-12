use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;

use crate::error::{AppError, AppResult};
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
    writer: Option<OwnedWriteHalf>,
}

impl TelnetStream {
    pub async fn connect(
        host: &str,
        port: u16,
        on_output: OutputSink,
        on_closed: ClosedSignal,
    ) -> AppResult<Self> {
        let stream = TcpStream::connect((host, port)).await
            .map_err(|e| AppError::Connection(e.to_string()))?;
        let (mut reader, writer) = stream.into_split();
        let mut parser = TelnetParser::new();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let clean = parser.feed(&buf[..n]);
                        if !clean.is_empty() {
                            on_output(&clean);
                        }
                    }
                }
            }
            on_closed();
        });
        Ok(TelnetStream { writer: Some(writer) })
    }
}

#[async_trait]
impl TerminalStream for TelnetStream {
    async fn write(&mut self, data: &[u8]) -> AppResult<()> {
        let w = self.writer.as_mut().ok_or_else(|| AppError::Connection("closed".into()))?;
        w.write_all(data).await.map_err(AppError::Io)?;
        Ok(())
    }

    async fn close(&mut self) -> AppResult<()> {
        if let Some(mut w) = self.writer.take() {
            let _ = w.shutdown().await;
        }
        Ok(())
    }
}

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
}
