use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;
use crate::protocol::{serial, ssh, telnet, ClosedSignal, OutputSink, TerminalStream};
use crate::session::manager::{SessionConfig, SessionId, SharedManager};
use crate::zmodem::{ZmodemRegistry, ZmodemState};

/// 认证失败事件负载。仅在会话由已保存档案启动（`config.profile_id` 非空）时发出：
/// 前端据此弹出凭据重输框，避免"档案里存着过期密码、界面上却无处可改"。
#[derive(Clone, serde::Serialize)]
pub struct AuthFailedEvent {
    pub session_id: SessionId,
    pub profile_id: String,
    pub message: String,
}

pub async fn connect_session(mgr: SharedManager, id: SessionId, cfg: SessionConfig, app: AppHandle) {
    let base_sink = make_output_sink(&app, mgr.clone(), id.clone());
    let sink = make_zmodem_sink(&app, base_sink, id.clone());
    let closed = make_closed_signal(&app, mgr.clone(), id.clone());

    let known_hosts = app
        .path()
        .app_data_dir()
        .map(|d| d.join("known_hosts"))
        .unwrap_or_else(|_| std::env::temp_dir().join("xtshell-known_hosts"));
    let host_key = ssh::HostKeyCheck {
        known_hosts,
        app: app.clone(),
        session_id: id.clone(),
        gate: app.state::<Arc<ssh::HostKeyGate>>().inner().clone(),
    };

    let result: Result<Box<dyn TerminalStream>, AppError> = match cfg.protocol.as_str() {
        "ssh" => match ssh::SshStream::connect(
            &cfg.host.clone().unwrap_or_default(),
            cfg.port.unwrap_or(22),
            &cfg.username.clone().unwrap_or_default(),
            cfg.password.clone(),
            cfg.private_key.clone(),
            cfg.private_key_passphrase.clone(),
            80,
            24,
            sink,
            closed,
            host_key,
        )
        .await
        {
            Ok(stream) => {
                // SFTP：注入句柄槽位（在既有 SSH 连接上开 sftp subsystem channel）
                let slot = stream.handle_slot();
                app.state::<crate::sftp::SftpRegistry>()
                    .register_handle(&id, slot);
                Ok(Box::new(stream))
            }
            Err(e) => Err(e),
        },
        "telnet" => telnet::TelnetStream::connect(
            &cfg.host.clone().unwrap_or_default(),
            cfg.port.unwrap_or(23),
            cfg.username.clone(),
            cfg.password.clone(),
            sink,
            closed,
        )
        .await
        .map(|s| Box::new(s) as Box<dyn TerminalStream>),
        "serial" => serial::SerialConn::open(
            &cfg.serial_port.clone().unwrap_or_default(),
            cfg.baud_rate.unwrap_or(9600),
            cfg.data_bits,
            cfg.stop_bits,
            cfg.parity.as_deref(),
            sink,
            closed,
        )
        .map(|s| Box::new(s) as Box<dyn TerminalStream>),
        other => Err(AppError::UnsupportedProtocol(other.to_string())),
    };

    match result {
        Ok(stream) => {
            let mut guard = mgr.lock().await;
            let _ = guard.attach_stream(&id, stream);
            let _ = app.emit("sessions-changed", ());
        }
        Err(e) => {
            let mut guard = mgr.lock().await;
            let _ = guard.inject_output_no_sink(&id, format!("{}\r\n", e).as_bytes());
            let _ = guard.mark_closed(&id);
            let _ = app.emit(&format!("session-error-{id}"), e.to_string());
            // 认证失败且凭据来自档案：额外通知前端重输（其余错误不回弹输入框）
            if let AppError::AuthFailed(message) = &e {
                if let Some(profile_id) = cfg.profile_id.clone() {
                    let _ = app.emit(
                        &format!("session-auth-failed-{id}"),
                        AuthFailedEvent {
                            session_id: id.clone(),
                            profile_id,
                            message: message.clone(),
                        },
                    );
                }
            }
            let _ = app.emit("sessions-changed", ());
        }
    }
}

fn make_output_sink(app: &AppHandle, mgr: SharedManager, id: SessionId) -> OutputSink {
    let app = app.clone();
    Arc::new(move |data: &[u8]| {
        let owned = data.to_vec();
        let _ = app.emit(&format!("session-output-{id}"), owned.clone());
        let mgr2 = mgr.clone();
        let id2 = id.clone();
        tokio::spawn(async move {
            let mut g = mgr2.lock().await;
            let _ = g.inject_output_no_sink(&id2, &owned);
        });
    })
}

/// ZMODEM 预过滤：检测起始序列并分流；透传部分交给基础 sink
fn make_zmodem_sink(app: &AppHandle, base: OutputSink, id: SessionId) -> OutputSink {
    let app = app.clone();
    Arc::new(move |data: &[u8]| {
        let entry = ZmodemState::entry(&app.state::<ZmodemRegistry>(), &id);
        let (passthrough, detected) = entry.lock().unwrap().feed(data);
        if detected {
            crate::zmodem::on_detected(&app, &id);
        }
        if !passthrough.is_empty() {
            base(&passthrough);
        }
    })
}

fn make_closed_signal(app: &AppHandle, mgr: SharedManager, id: SessionId) -> ClosedSignal {
    let app = app.clone();
    Box::new(move || {
        let _ = app.emit(&format!("session-state-{id}"), "closed");
        let _ = app.emit("sessions-changed", ());
        // 会话关闭：复位 zmodem 模式并终止泵；SFTP 会话一并失效
        crate::zmodem::reset_on_close(&app.state::<ZmodemRegistry>(), &id);
        crate::sftp::reset_on_close(&app.state::<crate::sftp::SftpRegistry>(), &id);
        // 若主机密钥确认仍在等待，按拒绝收敛
        app.state::<Arc<crate::protocol::ssh::HostKeyGate>>()
            .inner()
            .cancel(&id);
        let mgr2 = mgr.clone();
        let id2 = id.clone();
        tokio::spawn(async move {
            let mut g = mgr2.lock().await;
            let _ = g.mark_closed(&id2);
        });
    })
}
