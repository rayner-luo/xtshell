// SFTP：在既有 SSH 会话上开 sftp subsystem 通道，提供远程目录浏览与文件传输
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;

use crate::error::AppError;
use crate::protocol::ssh::SshHandleSlot;

#[derive(Default)]
pub struct SftpRegistry(pub Arc<Mutex<HashMap<String, SftpState>>>);

pub struct SftpState {
    handle_slot: Option<SshHandleSlot>,
    session: Option<Arc<SftpSession>>,
    cwd: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct SftpEntryDto {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: u64,
}

#[derive(Serialize, Clone)]
pub struct SftpProgress {
    pub kind: String,
    pub name: String,
    pub done: u64,
    pub total: u64,
}

fn emit_progress(app: &AppHandle, id: &str, kind: &str, name: &str, done: u64, total: u64) {
    let _ = app.emit(
        &format!("sftp-progress-{id}"),
        SftpProgress {
            kind: kind.into(),
            name: name.into(),
            done,
            total,
        },
    );
}

impl SftpRegistry {
    /// SSH 连接建立后由 connector 注入句柄槽位
    pub fn register_handle(&self, id: &str, slot: SshHandleSlot) {
        let mut map = self.0.lock().unwrap();
        let e = map.entry(id.to_string()).or_insert_with(|| SftpState {
            handle_slot: None,
            session: None,
            cwd: None,
        });
        e.handle_slot = Some(slot);
    }
}

/// 会话关闭：丢弃 SFTP 会话（底层 SSH 已断，句柄随之失效）
pub fn reset_on_close(reg: &SftpRegistry, id: &str) {
    let mut map = reg.0.lock().unwrap();
    if let Some(e) = map.get_mut(id) {
        e.session = None;
        e.cwd = None;
        e.handle_slot = None;
    }
}

/// 建立或复用 SFTP 会话（同一 SSH 连接上新开 sftp subsystem channel）
async fn ensure_connected(reg: &SftpRegistry, id: &str) -> Result<Arc<SftpSession>, AppError> {
    // 已连接：直接复用
    {
        let map = reg.0.lock().unwrap();
        if let Some(st) = map.get(id) {
            if let Some(s) = st.session.as_ref() {
                return Ok(s.clone());
            }
        }
    }
    // 取句柄槽位（仅 SSH 会话有）
    let slot = {
        let mut map = reg.0.lock().unwrap();
        let st = map
            .get_mut(id)
            .ok_or_else(|| AppError::Connection("not connected".into()))?;
        st.handle_slot
            .take()
            .ok_or_else(|| AppError::Connection("仅 SSH 会话支持 SFTP".into()))?
    };
    let session = open_sftp_session(slot).await?;
    let session = Arc::new(session);
    let mut map = reg.0.lock().unwrap();
    let st = map
        .get_mut(id)
        .ok_or_else(|| AppError::Connection("not connected".into()))?;
    // 期间可能已被并发连接
    if let Some(s) = st.session.as_ref() {
        return Ok(s.clone());
    }
    st.session = Some(session.clone());
    Ok(session)
}

async fn open_sftp_session(slot: SshHandleSlot) -> Result<SftpSession, AppError> {
    let mut guard = slot.lock().await;
    let handle = guard
        .as_mut()
        .ok_or_else(|| AppError::Connection("not connected".into()))?;
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| AppError::Connection(format!("sftp channel: {e}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|e| AppError::Connection(format!("sftp subsystem: {e}")))?;
    SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| AppError::Connection(format!("sftp init: {e}")))
}

/// 显式断开 SFTP（SSH 会话保持）
pub async fn disconnect(reg: &SftpRegistry, id: &str) {
    let session = {
        let mut map = reg.0.lock().unwrap();
        match map.get_mut(id) {
            Some(st) => st.session.take(),
            None => None,
        }
    };
    if let Some(s) = session {
        let _ = s.close().await;
    }
}

fn dto_from(entry: &russh_sftp::client::fs::DirEntry) -> SftpEntryDto {
    SftpEntryDto {
        name: entry.file_name(),
        is_dir: entry.file_type().is_dir(),
        size: entry.metadata().size.unwrap_or(0),
        mtime: entry.metadata().mtime.unwrap_or(0) as u64,
    }
}

/// 连接并返回初始目录列表（默认根目录 /）
pub async fn connect(
    reg: &SftpRegistry,
    id: &str,
    path: Option<String>,
) -> Result<(String, Vec<SftpEntryDto>), AppError> {
    let sftp = ensure_connected(reg, id).await?;
    let start = match &path {
        Some(p) => p.clone(),
        None => "/".to_string(),
    };
    let list = list_inner(&sftp, &start).await?;
    let mut map = reg.0.lock().unwrap();
    if let Some(st) = map.get_mut(id) {
        st.cwd = Some(start.clone());
    }
    Ok((start, list))
}

fn sftp_err(e: russh_sftp::client::error::Error) -> AppError {
    AppError::Connection(format!("sftp: {e}"))
}

async fn list_inner(sftp: &SftpSession, path: &str) -> Result<Vec<SftpEntryDto>, AppError> {
    let rd = sftp.read_dir(path).await.map_err(sftp_err)?;
    let mut out: Vec<SftpEntryDto> = rd.map(|e| dto_from(&e)).collect();
    // 目录在前、按名称排序
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

/// 列目录（复用已建立的 SFTP 会话）
pub async fn list(reg: &SftpRegistry, id: &str, path: &str) -> Result<Vec<SftpEntryDto>, AppError> {
    let sftp = ensure_connected(reg, id).await?;
    list_inner(&sftp, path).await
}

pub async fn mkdir(reg: &SftpRegistry, id: &str, path: &str) -> Result<(), AppError> {
    let sftp = ensure_connected(reg, id).await?;
    sftp.create_dir(path).await.map_err(sftp_err)
}

pub async fn rename(reg: &SftpRegistry, id: &str, old: &str, new: &str) -> Result<(), AppError> {
    let sftp = ensure_connected(reg, id).await?;
    sftp.rename(old, new).await.map_err(sftp_err)
}

/// 删除文件或目录（目录递归删除；调用方负责确认交互）
pub async fn remove(reg: &SftpRegistry, id: &str, path: &str, is_dir: bool) -> Result<(), AppError> {
    let sftp = ensure_connected(reg, id).await?;
    if is_dir {
        remove_dir_recursive(&sftp, path).await
    } else {
        sftp.remove_file(path).await.map_err(sftp_err)
    }
}

async fn remove_dir_recursive(sftp: &SftpSession, path: &str) -> Result<(), AppError> {
    let rd = sftp.read_dir(path).await.map_err(sftp_err)?;
    for entry in rd {
        let child = format!("{}/{}", path.trim_end_matches('/'), entry.file_name());
        if entry.file_type().is_dir() {
            Box::pin(remove_dir_recursive(sftp, &child)).await?;
        } else {
            sftp.remove_file(&child).await.map_err(sftp_err)?;
        }
    }
    sftp.remove_dir(path).await.map_err(sftp_err)
}

/// 上传前冲突预检：返回远端目录下已存在的文件名
pub async fn check_upload(
    reg: &SftpRegistry,
    id: &str,
    remote_dir: &str,
    names: &[String],
) -> Result<Vec<String>, AppError> {
    let sftp = ensure_connected(reg, id).await?;
    let mut conflicts = Vec::new();
    for name in names {
        let remote = join_path(remote_dir, name);
        if sftp.try_exists(&remote).await.map_err(sftp_err)? {
            conflicts.push(name.clone());
        }
    }
    Ok(conflicts)
}

/// 批量上传；policy: overwrite | skip（前端冲突询问后下发）
pub async fn upload(
    app: &AppHandle,
    reg: &SftpRegistry,
    id: &str,
    remote_dir: &str,
    local_paths: &[String],
    policy: &str,
) -> Result<(u32, u32), AppError> {
    let sftp = ensure_connected(reg, id).await?;
    let mut done = 0u32;
    let mut skipped = 0u32;
    for lp in local_paths {
        let name = std::path::Path::new(lp)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let remote = join_path(remote_dir, &name);
        if policy != "overwrite" {
            let exists = sftp.try_exists(&remote).await.map_err(sftp_err)?;
            if exists {
                skipped += 1;
                emit_progress(app, id, "skip", &name, 0, 0);
                continue;
            }
        }
        let total = tokio::fs::metadata(lp).await.map(|m| m.len()).unwrap_or(0);
        emit_progress(app, id, "start", &name, 0, total);
        let mut local = tokio::fs::File::open(lp)
            .await
            .map_err(AppError::Io)?;
        let mut remote_file = sftp
            .open_with_flags(&remote, OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE)
            .await
            .map_err(sftp_err)?;
        let mut buf = vec![0u8; 32 * 1024];
        let mut sent: u64 = 0;
        loop {
            let n = local.read(&mut buf).await.map_err(AppError::Io)?;
            if n == 0 {
                break;
            }
            remote_file
                .write_all(&buf[..n])
                .await
                .map_err(AppError::Io)?;
            sent += n as u64;
            emit_progress(app, id, "progress", &name, sent, total);
        }
        remote_file.shutdown().await.map_err(AppError::Io)?;
        done += 1;
        emit_progress(app, id, "done", &name, total, total);
    }
    Ok((done, skipped))
}

/// 下载到本地路径（流式 + 进度事件）
pub async fn download(
    app: &AppHandle,
    reg: &SftpRegistry,
    id: &str,
    remote: &str,
    local: &str,
) -> Result<u64, AppError> {
    let sftp = ensure_connected(reg, id).await?;
    let meta = sftp.metadata(remote).await.map_err(sftp_err)?;
    let total = meta.size.unwrap_or(0);
    let name = std::path::Path::new(remote)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    emit_progress(app, id, "start", &name, 0, total);
    let mut remote_file = sftp.open(remote).await.map_err(sftp_err)?;
    let mut local_file = tokio::fs::File::create(local).await.map_err(AppError::Io)?;
    let mut buf = vec![0u8; 32 * 1024];
    let mut got: u64 = 0;
    let mut last_pct = 255u8;
    loop {
        let n = remote_file.read(&mut buf).await.map_err(AppError::Io)?;
        if n == 0 {
            break;
        }
        local_file.write_all(&buf[..n]).await.map_err(AppError::Io)?;
        got += n as u64;
        if total > 0 {
            let pct = (got * 100 / total) as u8;
            if pct != last_pct {
                last_pct = pct;
                emit_progress(app, id, "progress", &name, got, total);
            }
        }
    }
    emit_progress(app, id, "done", &name, got, total);
    Ok(got)
}

fn join_path(dir: &str, name: &str) -> String {
    format!("{}/{}", dir.trim_end_matches('/'), name)
}
