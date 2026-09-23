use std::time::Duration;

use async_trait::async_trait;
use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use russh::{Preferred, cipher as ssh_cipher, kex as ssh_kex, mac as ssh_mac};

use crate::error::{AppError, AppResult};
use crate::protocol::{ClosedSignal, OutputSink, TerminalStream};

pub struct ClientHandler;
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
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
        let mut handle = client::connect(config, (host, port), ClientHandler).await
            .map_err(|e| connect_err(e, host))?;

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
                .map_err(|e| AppError::Connection(e.to_string()))?;
            let result = handle
                .authenticate_publickey(username, PrivateKeyWithHashAlg::new(std::sync::Arc::new(key), None))
                .await
                .map_err(|e| AppError::Connection(e.to_string()))?;
            if !result.success() {
                return Err(AppError::Connection("public key authentication failed".into()));
            }
            return Ok(());
        }
        if let Some(password) = password {
            let result = handle.authenticate_password(username, password).await
                .map_err(|e| AppError::Connection(e.to_string()))?;
            if !result.success() {
                return Err(AppError::Connection("password authentication failed".into()));
            }
            return Ok(());
        }
        Err(AppError::Connection("no auth method provided".into()))
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
