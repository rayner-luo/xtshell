use tauri::{AppHandle, Emitter, State};

use serde::Serialize;

#[cfg(target_os = "linux")]
use gtk::prelude::ObjectExt;

use crate::error::ApiError;
use crate::profile::{ProfilesTree, SaveProfileRequest, SharedProfiles, TerminalAppearance};
use crate::session::manager::{SessionConfig, SessionId, SessionInfo, SharedManager};

#[derive(Debug, Serialize)]
pub struct SerialPortInfoDto {
    pub path: String,
    pub kind: String,
}

/// 主题切换同步到原生层：Linux 上 select 下拉/右键菜单等由 GTK 原生渲染，
/// 运行时切换 GtkSettings 主题使其跟随应用主题；其他平台由网页层 color-scheme 自管。
/// 注意：不能用 gtk-application-prefer-dark-theme——webview 存在后 false→true
/// 切换会导致 WebKitGTK 的下拉弹出窗口无法创建（实测），改设完整主题名则无此问题。
#[tauri::command]
pub fn set_native_theme(dark: bool) {
    #[cfg(target_os = "linux")]
    if let Some(settings) = gtk::Settings::default() {
        settings.set_property(
            "gtk-theme-name",
            if dark { "Adwaita-dark" } else { "Adwaita" },
        );
    }
    #[cfg(not(target_os = "linux"))]
    let _ = dark;
}

/// 应用日志（启动日志）：返回日志文件路径与最近若干行，**最新在最前**。
#[derive(Debug, Serialize)]
pub struct AppLogDto {
    pub path: String,
    pub lines: Vec<String>,
}

#[tauri::command]
pub fn read_app_log(limit: Option<usize>) -> AppLogDto {
    AppLogDto {
        path: crate::applog::path()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        lines: crate::applog::tail(limit.unwrap_or(2000)),
    }
}

#[tauri::command]
pub fn clear_app_log() {
    crate::applog::clear();
}

#[tauri::command]
pub fn list_serial_ports() -> Result<Vec<SerialPortInfoDto>, ApiError> {
    let ports = tokio_serial::available_ports()
        .map_err(|e| ApiError::from(crate::error::AppError::Connection(e.to_string())))?;
    Ok(ports
        .into_iter()
        .map(|p| SerialPortInfoDto {
            path: p.port_name,
            kind: match p.port_type {
                tokio_serial::SerialPortType::UsbPort(info) => format!("USB ({})", info.product.clone().unwrap_or_default()),
                tokio_serial::SerialPortType::BluetoothPort => "Bluetooth".into(),
                tokio_serial::SerialPortType::PciPort => "PCI".into(),
                _ => "Unknown".into(),
            },
        })
        .collect())
}

#[tauri::command]
pub async fn create_session(
    app: AppHandle,
    mgr: State<'_, SharedManager>,
    config: SessionConfig,
) -> Result<SessionId, ApiError> {
    let id = mgr.lock().await.create(config.clone());
    let _ = app.emit("sessions-changed", ());
    let mgr2 = (*mgr).clone();
    let id2 = id.clone();
    tauri::async_runtime::spawn(crate::connector::connect_session(mgr2, id2, config, app));
    Ok(id)
}

#[tauri::command]
pub async fn close_session(
    app: AppHandle,
    mgr: State<'_, SharedManager>,
    gate: State<'_, std::sync::Arc<crate::protocol::ssh::HostKeyGate>>,
    id: SessionId,
) -> Result<(), ApiError> {
    // 锁内只做同步移除；stream.close() 在锁外 await（可能涉及网络往返）
    let stream = mgr
        .lock()
        .await
        .take_for_close(&id)
        .map_err(ApiError::from)?;
    gate.cancel(&id);
    if let Some(mut stream) = stream {
        stream.close().await.map_err(ApiError::from)?;
    }
    let _ = app.emit("sessions-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn list_sessions(mgr: State<'_, SharedManager>) -> Result<Vec<SessionInfo>, ApiError> {
    Ok(mgr.lock().await.list())
}

/// 首次连接 SSH 主机时由前端投递确认结果（接受后写入 known_hosts）。
#[tauri::command]
pub fn ssh_hostkey_decision(
    gate: State<'_, std::sync::Arc<crate::protocol::ssh::HostKeyGate>>,
    id: String,
    accept: bool,
) {
    gate.resolve(&id, accept);
}

#[tauri::command]
pub async fn send_input(
    mgr: State<'_, SharedManager>,
    id: SessionId,
    data: String,
) -> Result<(), ApiError> {
    // 输入指令缓存（sz/rz 方向 hint），再直写流
    crate::zmodem::note_input(&id, &data);
    mgr.lock()
        .await
        .send_input(&id, data.as_bytes())
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn resize_session(
    mgr: State<'_, SharedManager>,
    id: SessionId,
    cols: u32,
    rows: u32,
) -> Result<(), ApiError> {
    mgr.lock()
        .await
        .resize(&id, cols, rows)
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn get_session_output(
    mgr: State<'_, SharedManager>,
    id: SessionId,
    offset: u64,
) -> Result<(Vec<u8>, u64), ApiError> {
    mgr.lock()
        .await
        .get_output(&id, offset)
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn reconnect_session(
    app: AppHandle,
    mgr: State<'_, SharedManager>,
    id: SessionId,
) -> Result<SessionId, ApiError> {
    let config = mgr.lock().await.take_closed(&id).map_err(ApiError::from)?;
    let new_id = mgr.lock().await.create(config.clone());
    let _ = app.emit("sessions-changed", ());
    let mgr2 = (*mgr).clone();
    let id2 = new_id.clone();
    tauri::async_runtime::spawn(crate::connector::connect_session(mgr2, id2, config, app));
    Ok(new_id)
}

#[tauri::command]
pub async fn set_session_log(
    mgr: State<'_, SharedManager>,
    id: SessionId,
    path: Option<String>,
) -> Result<Option<String>, ApiError> {
    let mut guard = mgr.lock().await;
    match path {
        Some(path) => {
            // 文件名由前端在「另存为」里指定（默认 `年月日时分秒.txt`）
            let saved = guard.start_log(&id, &path).map_err(ApiError::from)?;
            Ok(Some(saved))
        }
        None => {
            guard.stop_log(&id).map_err(ApiError::from)?;
            Ok(None)
        }
    }
}

fn changed(app: &AppHandle, event: &str) {
    let _ = app.emit(event, ());
}

#[tauri::command]
pub async fn list_profiles(profiles: State<'_, SharedProfiles>) -> Result<ProfilesTree, ApiError> {
    Ok(profiles.lock().await.tree())
}

#[tauri::command]
pub async fn save_profile(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    req: SaveProfileRequest,
) -> Result<String, ApiError> {
    let id = profiles.lock().await.upsert_session(req).map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(id)
}

#[tauri::command]
pub async fn update_profile_appearance(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    id: String,
    appearance: TerminalAppearance,
) -> Result<(), ApiError> {
    profiles
        .lock()
        .await
        .set_terminal_appearance(&id, appearance)
        .map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(())
}

#[tauri::command]
pub async fn delete_profile(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    id: String,
) -> Result<(), ApiError> {
    profiles.lock().await.delete_session(&id).map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(())
}

#[tauri::command]
/// 从已保存 profile 启动会话（Tauri command 与 MCP 共用）。
/// 已保存凭据由 build_session_config 自动解密；显式参数仅作覆盖。
pub async fn launch_session_from_profile(
    app: AppHandle,
    profiles: SharedProfiles,
    mgr: SharedManager,
    id: String,
    password: Option<String>,
    passphrase: Option<String>,
    private_key: Option<String>,
) -> Result<SessionId, ApiError> {
    let mut config = profiles
        .lock()
        .await
        .build_session_config(&id)
        .map_err(ApiError::from)?;
    // 连接前弹框收集的认证信息覆盖（仅当提供时）
    if password.is_some() {
        config.password = password;
    }
    if passphrase.is_some() {
        config.private_key_passphrase = passphrase;
    }
    if private_key.is_some() {
        config.private_key = private_key;
    }
    let session_id = mgr.lock().await.create(config.clone());
    // 立即通知前端刷新：连接中的 tab 即时可见
    let _ = app.emit("sessions-changed", ());
    let mgr2 = mgr.clone();
    let id2 = session_id.clone();
    tauri::async_runtime::spawn(crate::connector::connect_session(mgr2, id2, config, app));
    Ok(session_id)
}

#[tauri::command]
pub async fn launch_profile(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    mgr: State<'_, SharedManager>,
    id: String,
    password: Option<String>,
    passphrase: Option<String>,
    private_key: Option<String>,
) -> Result<SessionId, ApiError> {
    launch_session_from_profile(app, (*profiles).clone(), (*mgr).clone(), id, password, passphrase, private_key).await
}

#[tauri::command]
pub async fn set_profile_secret(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    id: String,
    password: Option<String>,
    passphrase: Option<String>,
    private_key: Option<String>,
) -> Result<(), ApiError> {
    profiles
        .lock()
        .await
        .set_profile_secret(&id, password, passphrase, private_key)
        .map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(())
}

#[tauri::command]
pub async fn create_folder(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    name: String,
    parent: Option<String>,
) -> Result<String, ApiError> {
    let id = profiles
        .lock()
        .await
        .create_folder(&name, parent)
        .map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(id)
}

#[tauri::command]
pub async fn rename_folder(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    id: String,
    name: String,
) -> Result<(), ApiError> {
    profiles
        .lock()
        .await
        .rename_folder(&id, &name)
        .map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(())
}

#[tauri::command]
pub async fn delete_folder(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    id: String,
) -> Result<(), ApiError> {
    profiles.lock().await.delete_folder(&id).map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(())
}

#[tauri::command]
pub async fn move_profile(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    id: String,
    folder: Option<String>,
) -> Result<(), ApiError> {
    profiles
        .lock()
        .await
        .move_session(&id, folder)
        .map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(())
}

#[tauri::command]
pub async fn move_folder(
    app: AppHandle,
    profiles: State<'_, SharedProfiles>,
    id: String,
    parent: Option<String>,
) -> Result<(), ApiError> {
    profiles
        .lock()
        .await
        .move_folder(&id, parent)
        .map_err(ApiError::from)?;
    changed(&app, "profiles-changed");
    Ok(())
}

#[tauri::command]
pub async fn zmodem_start_receive(
    app: AppHandle,
    mgr: State<'_, SharedManager>,
    reg: State<'_, crate::zmodem::ZmodemRegistry>,
    id: SessionId,
    dir: String,
) -> Result<(), ApiError> {
    crate::zmodem::start_receiver(app, (*mgr).clone(), &reg, &id, Some(dir))
        .map_err(ApiError::from)?;
    Ok(())
}

#[tauri::command]
pub async fn zmodem_send_files(
    app: AppHandle,
    mgr: State<'_, SharedManager>,
    reg: State<'_, crate::zmodem::ZmodemRegistry>,
    id: SessionId,
    paths: Vec<String>,
) -> Result<(), ApiError> {
    crate::zmodem::start_sender(app, (*mgr).clone(), &reg, &id, paths)
        .map_err(|e| ApiError::from(e))?;
    Ok(())
}

#[tauri::command]
pub async fn zmodem_resolve_conflict(
    temp: String,
    dest: String,
    choice: String,
) -> Result<(), ApiError> {
    // 冲突裁决：overwrite=覆盖 / rename=自动改名 / skip=删除临时文件
    match choice.as_str() {
        "overwrite" => {
            if tokio::fs::rename(&temp, &dest).await.is_ok() {
                return Ok(());
            }
            tokio::fs::copy(&temp, &dest)
                .await
                .map_err(|e| ApiError::from(crate::error::AppError::Connection(e.to_string())))?;
            let _ = tokio::fs::remove_file(&temp).await;
            Ok(())
        }
        "rename" => {
            let path = std::path::Path::new(&dest);
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{e}"))
                .unwrap_or_default();
            let parent = path.parent().unwrap_or(std::path::Path::new("."));
            let mut n = 1;
            loop {
                let candidate = parent.join(format!("{stem} ({n}){ext}", stem = stem, n = n, ext = ext));
                if !candidate.exists() {
                    let dest_str = candidate.to_string_lossy().into_owned();
                    if tokio::fs::rename(&temp, &dest_str).await.is_ok() {
                        return Ok(());
                    }
                    tokio::fs::copy(&temp, &dest_str)
                        .await
                        .map_err(|e| ApiError::from(crate::error::AppError::Connection(e.to_string())))?;
                    let _ = tokio::fs::remove_file(&temp).await;
                    return Ok(());
                }
                n += 1;
            }
        }
        "skip" => {
            let _ = tokio::fs::remove_file(&temp).await;
            Ok(())
        }
        _ => Err(ApiError::from(crate::error::AppError::Connection(
            "invalid conflict choice".into(),
        ))),
    }
}

#[tauri::command]
pub async fn zmodem_cancel(
    app: AppHandle,
    mgr: State<'_, SharedManager>,
    reg: State<'_, crate::zmodem::ZmodemRegistry>,
    id: SessionId,
) -> Result<(), ApiError> {
    crate::zmodem::cancel_transfer(app, (*mgr).clone(), &reg, &id).await;
    Ok(())
}

// ---------- SFTP ----------

#[tauri::command]
pub async fn sftp_connect(
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
    path: Option<String>,
) -> Result<(String, Vec<crate::sftp::SftpEntryDto>), ApiError> {
    crate::sftp::connect(&reg, &id, path)
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn sftp_disconnect(
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
) -> Result<(), ApiError> {
    crate::sftp::disconnect(&reg, &id).await;
    Ok(())
}

#[tauri::command]
pub async fn sftp_list(
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
    path: String,
) -> Result<Vec<crate::sftp::SftpEntryDto>, ApiError> {
    crate::sftp::list(&reg, &id, &path)
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn sftp_mkdir(
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
    path: String,
) -> Result<(), ApiError> {
    crate::sftp::mkdir(&reg, &id, &path)
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn sftp_remove(
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
    path: String,
    is_dir: bool,
) -> Result<(), ApiError> {
    crate::sftp::remove(&reg, &id, &path, is_dir)
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn sftp_rename(
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
    old_path: String,
    new_path: String,
) -> Result<(), ApiError> {
    crate::sftp::rename(&reg, &id, &old_path, &new_path)
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn sftp_check_upload(
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
    remote_dir: String,
    names: Vec<String>,
) -> Result<Vec<String>, ApiError> {
    crate::sftp::check_upload(&reg, &id, &remote_dir, &names)
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn sftp_upload(
    app: AppHandle,
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
    remote_dir: String,
    local_paths: Vec<String>,
    policy: String,
) -> Result<(u32, u32), ApiError> {
    crate::sftp::upload(&app, &reg, &id, &remote_dir, &local_paths, &policy)
        .await
        .map_err(ApiError::from)
}

#[tauri::command]
pub async fn sftp_download(
    app: AppHandle,
    reg: State<'_, crate::sftp::SftpRegistry>,
    id: SessionId,
    remote: String,
    local: String,
) -> Result<u64, ApiError> {
    crate::sftp::download(&app, &reg, &id, &remote, &local)
        .await
        .map_err(ApiError::from)
}
