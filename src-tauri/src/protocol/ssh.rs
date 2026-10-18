use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use russh::{Preferred, cipher as ssh_cipher, kex as ssh_kex, mac as ssh_mac};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::protocol::{ClosedSignal, OutputSink, TerminalStream};

/// 等待前端确认的主机密钥（首次连接）。
#[derive(Clone, Serialize)]
pub struct HostKeyPrompt {
    pub session_id: String,
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
    pub known_hosts: String,
}

/// 主机密钥确认的等待槽：SSH 握手任务在这里挂起，前端通过
/// `ssh_hostkey_decision` 命令投递接受/拒绝。
#[derive(Default)]
pub struct HostKeyGate {
    pending: Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
}

impl HostKeyGate {
    fn register(&self, id: &str) -> tokio::sync::oneshot::Receiver<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending.lock().unwrap().insert(id.to_string(), tx);
        rx
    }

    /// 投递确认结果；返回是否命中了一个等待中的请求。
    pub fn resolve(&self, id: &str, accept: bool) -> bool {
        match self.pending.lock().unwrap().remove(id) {
            Some(tx) => tx.send(accept).is_ok(),
            None => false,
        }
    }

    /// 会话被关闭时取消仍在等待的确认（按拒绝处理）。
    pub fn cancel(&self, id: &str) {
        self.resolve(id, false);
    }
}

/// 主机密钥校验上下文：known_hosts 路径、用于弹确认框的通道。
pub struct HostKeyCheck {
    pub known_hosts: std::path::PathBuf,
    pub app: tauri::AppHandle,
    pub session_id: String,
    pub gate: Arc<HostKeyGate>,
}

pub struct ClientHandler {
    host: String,
    port: u16,
    check: HostKeyCheck,
    /// 校验失败的具体原因（russh 只回 UnknownKey，需要单独带回给用户）
    rejection: Arc<Mutex<Option<String>>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        match russh::keys::check_known_hosts_path(
            &self.host,
            self.port,
            server_public_key,
            &self.check.known_hosts,
        ) {
            Ok(true) => return Ok(true),
            Ok(false) => {} // 未知主机：走一次用户确认（TOFU）
            Err(russh::keys::Error::KeyChanged { line }) => {
                *self.rejection.lock().unwrap() = Some(format!(
                    "主机密钥已变更（known_hosts 第 {line} 行）。可能存在中间人攻击；\
                     如确认设备已更换密钥，请删除 {} 中对应条目后重试。",
                    self.check.known_hosts.display()
                ));
                return Ok(false);
            }
            Err(e) => {
                *self.rejection.lock().unwrap() =
                    Some(format!("读取 known_hosts 失败：{e}"));
                return Ok(false);
            }
        }

        let fingerprint = server_public_key
            .fingerprint(russh::keys::ssh_key::HashAlg::Sha256)
            .to_string();
        let rx = self.check.gate.register(&self.check.session_id);
        let _ = tauri::Emitter::emit(
            &self.check.app,
            "ssh-hostkey-prompt",
            HostKeyPrompt {
                session_id: self.check.session_id.clone(),
                host: self.host.clone(),
                port: self.port,
                key_type: server_public_key.algorithm().as_str().to_string(),
                fingerprint,
                known_hosts: self.check.known_hosts.display().to_string(),
            },
        );

        let accepted = match tokio::time::timeout(Duration::from_secs(120), rx).await {
            Ok(Ok(accept)) => accept,
            _ => {
                // 超时/通道断开：清掉槽位，避免残留 sender
                self.check.gate.cancel(&self.check.session_id);
                false
            }
        };

        if !accepted {
            *self.rejection.lock().unwrap() =
                Some("已取消主机密钥确认，连接中止".to_string());
            return Ok(false);
        }

        if let Err(e) = russh::keys::known_hosts::learn_known_hosts_path(
            &self.host,
            self.port,
            server_public_key,
            &self.check.known_hosts,
        ) {
            *self.rejection.lock().unwrap() =
                Some(format!("写入 known_hosts 失败：{e}"));
            return Ok(false);
        }
        Ok(true)
    }
}

/// 将 russh 连接错误映射为友好提示：底层 IO 超时（WSA 10060 / Linux 110）统一为
/// AppError::ConnectTimeout（"连接超时，请重试"），其余保留原文。
fn connect_err(e: russh::Error, host: &str) -> AppError {
    match e {
        russh::Error::IO(io) => AppError::map_connect_io(io, host),
        other => AppError::Connection(other.to_string()),
    }
}

/// SSH client 句柄共享槽位：终端流与 SFTP 通道共用（close 时取走并断开）
pub type SshHandleSlot = std::sync::Arc<tokio::sync::Mutex<Option<client::Handle<ClientHandler>>>>;

pub struct SshStream {
    write_half: Option<russh::ChannelWriteHalf<russh::client::Msg>>,
    session: SshHandleSlot,
}

impl SshStream {
    #[allow(clippy::too_many_arguments)]
    pub async fn connect(
        host: &str,
        port: u16,
        username: &str,
        password: Option<String>,
        private_key: Option<String>,
        private_key_passphrase: Option<String>,
        cols: u32,
        rows: u32,
        on_output: OutputSink,
        on_closed: ClosedSignal,
        host_key: HostKeyCheck,
    ) -> AppResult<Self> {
        let mut preferred = Preferred::default();
        preferred.kex.to_mut().extend([
            ssh_kex::DH_G14_SHA1,
            ssh_kex::DH_GEX_SHA1,
            ssh_kex::DH_G1_SHA1,
        ]);
        preferred.cipher.to_mut().extend([
            ssh_cipher::AES_192_CBC,
            ssh_cipher::AES_128_CBC,
            ssh_cipher::AES_256_CBC,
            ssh_cipher::TRIPLE_DES_CBC,
        ]);
        preferred
            .mac
            .to_mut()
            .extend([ssh_mac::HMAC_SHA1_ETM, ssh_mac::HMAC_SHA1]);
        let config = std::sync::Arc::new(client::Config {
            preferred,
            inactivity_timeout: Some(Duration::from_secs(60)),
            keepalive_interval: Some(Duration::from_secs(30)),
            ..Default::default()
        });
        let rejection = Arc::new(Mutex::new(None));
        let handler = ClientHandler {
            host: host.to_string(),
            port,
            check: host_key,
            rejection: rejection.clone(),
        };
        let mut handle = match client::connect(config, (host, port), handler).await {
            Ok(handle) => handle,
            Err(e) => {
                // 主机密钥校验失败时带回具体原因（russh 只报 UnknownKey）
                return Err(match rejection.lock().unwrap().take() {
                    Some(reason) => AppError::Connection(reason),
                    None => connect_err(e, host),
                });
            }
        };

        Self::authenticate(&mut handle, username, password, private_key, private_key_passphrase).await?;

        let channel = handle.channel_open_session().await
            .map_err(|e| AppError::Connection(e.to_string()))?;
        channel.request_pty(false, "xterm-256color", cols, rows, 0, 0, &[]).await
            .map_err(|e| AppError::Connection(e.to_string()))?;
        channel.request_shell(false).await
            .map_err(|e| AppError::Connection(e.to_string()))?;

        let (mut read_half, write_half) = channel.split();
        tokio::spawn(async move {
            loop {
                let msg = read_half.wait().await;
                match msg {
                    // ExtendedData is stderr; merge it into terminal output.
                    Some(russh::ChannelMsg::Data { data })
                    | Some(russh::ChannelMsg::ExtendedData { data, .. }) => on_output(&data),
                    Some(russh::ChannelMsg::ExitStatus { .. })
                    | Some(russh::ChannelMsg::Eof)
                    | Some(russh::ChannelMsg::Close) => break,
                    Some(_) => {}
                    None => break,
                }
            }
            on_closed();
        });

        Ok(SshStream {
            write_half: Some(write_half),
            session: std::sync::Arc::new(tokio::sync::Mutex::new(Some(handle))),
        })
    }

    /// 句柄槽位（供 SFTP 等子系统在既有 SSH 连接上开新 channel）
    pub fn handle_slot(&self) -> SshHandleSlot {
        self.session.clone()
    }

    async fn authenticate(
        handle: &mut client::Handle<ClientHandler>,
        username: &str,
        password: Option<String>,
        private_key: Option<String>,
        private_key_passphrase: Option<String>,
    ) -> AppResult<()> {
        if let Some(key_path) = private_key {
            let key = russh::keys::load_secret_key(&key_path, private_key_passphrase.as_deref())
                .map_err(|e| AppError::AuthFailed(e.to_string()))?;
            let result = handle
                .authenticate_publickey(username, PrivateKeyWithHashAlg::new(std::sync::Arc::new(key), None))
                .await
                .map_err(|e| AppError::Connection(e.to_string()))?;
            if !result.success() {
                return Err(AppError::AuthFailed("public key authentication failed".into()));
            }
            return Ok(());
        }
        if let Some(password) = password {
            let result = handle.authenticate_password(username, password).await
                .map_err(|e| AppError::Connection(e.to_string()))?;
            if !result.success() {
                return Err(AppError::AuthFailed("password authentication failed".into()));
            }
            return Ok(());
        }
        Err(AppError::AuthFailed("no auth method provided".into()))
    }
}

#[async_trait]
impl TerminalStream for SshStream {
    async fn write(&mut self, data: &[u8]) -> AppResult<()> {
        let half = self.write_half.as_ref().ok_or_else(|| AppError::Connection("closed".into()))?;
        half.data(data).await
            .map_err(|e| AppError::Connection(e.to_string()))
    }

    async fn resize(&mut self, cols: u32, rows: u32) -> AppResult<()> {
        let half = self.write_half.as_ref().ok_or_else(|| AppError::Connection("closed".into()))?;
        half.window_change(cols, rows, 0, 0).await
            .map_err(|e| AppError::Connection(e.to_string()))
    }

    async fn close(&mut self) -> AppResult<()> {
        if let Some(half) = self.write_half.take() {
            let _ = half.close().await;
        }
        let mut guard = self.session.lock().await;
        if let Some(handle) = guard.take() {
            let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "English").await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_accepts_matching_decision() {
        let gate = HostKeyGate::default();
        let mut rx = gate.register("s1");
        assert!(gate.resolve("s1", true));
        assert_eq!(rx.try_recv(), Ok(true));
    }

    #[tokio::test]
    async fn gate_delivers_rejection() {
        let gate = HostKeyGate::default();
        let rx = gate.register("s2");
        assert!(gate.resolve("s2", false));
        assert_eq!(rx.await, Ok(false));
    }

    #[test]
    fn gate_cancel_counts_as_reject() {
        let gate = HostKeyGate::default();
        let mut rx = gate.register("s3");
        gate.cancel("s3");
        assert_eq!(rx.try_recv(), Ok(false));
    }

    #[test]
    fn unknown_decision_is_ignored() {
        let gate = HostKeyGate::default();
        assert!(!gate.resolve("missing", true));
    }
}
