// ZMODEM（lrzsz）支持：检测 → 用户决策 → 收/发状态机驱动
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use zmodem2::{Action, Event, FileInfo, Position, Receiver, Sender};

use crate::error::AppError;
use crate::session::manager::{SessionId, SharedManager};

/// ZRQINIT 起始序列针（sz 实测为 00 型二进制，rz 实测为 01 型含 hex 变体）
const NEEDLES: &[&[u8]] = &[
    b"*\x18B00",           // 二进制帧头，帧类型 00（sz）
    b"*\x18B01",           // 二进制变体（lrzsz rz）
    b"**B0100000023be50",  // hex 帧（rz 实测：**B01 + 000000 + 23be + 50）
    b"**B0000000023be50",  // hex 变体
];

// ---------- 输入指令缓存：按行解析 sz/rz，检测后零等待分流 ----------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ZmodemHint {
    /// 远端 sz：远端发送 → 本端下载（弹目录选择）
    Download,
    /// 远端 rz：远端接收 → 本端上传（弹文件选择）
    Upload,
}

struct LineState {
    buf: String,
    hint: Option<ZmodemHint>,
}

static INPUT_LINES: std::sync::OnceLock<Mutex<HashMap<String, LineState>>> =
    std::sync::OnceLock::new();

/// 记录本端键盘输入（send_input 路径），按行解析 zmodem 命令
pub fn note_input(id: &str, data: &str) {
    let map = INPUT_LINES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map.lock().unwrap();
    let st = map
        .entry(id.to_string())
        .or_insert_with(|| LineState {
            buf: String::new(),
            hint: None,
        });
    for ch in data.chars() {
        match ch {
            '\r' | '\n' => {
                let line = std::mem::take(&mut st.buf);
                st.hint = classify_line(&line);
            }
            '\x7f' | '\x08' => {
                st.buf.pop();
            }
            '\x15' => st.buf.clear(), // Ctrl-U 清行
            '\x03' => {
                st.buf.clear(); // Ctrl-C
                st.hint = None;
            }
            c if c.is_ascii_graphic() || c == ' ' => {
                if st.buf.len() < 512 {
                    st.buf.push(c);
                }
            }
            _ => {} // ESC 序列等忽略（↑ 历史召回落到兜底分类）
        }
    }
}

fn classify_line(line: &str) -> Option<ZmodemHint> {
    let first = line.trim().split_whitespace().next()?;
    match first {
        "sz" | "sb" | "sx" | "tsz" => Some(ZmodemHint::Download),
        "rz" | "rb" | "rx" | "trz" => Some(ZmodemHint::Upload),
        _ => None,
    }
}

/// 取走 hint（用后即清，防陈旧误导）
fn take_hint(id: &str) -> Option<ZmodemHint> {
    let map = INPUT_LINES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map.lock().unwrap();
    map.get_mut(id).and_then(|st| st.hint.take())
}

/// 检测到 ZRQINIT 后的路由：有 hint → 直接弹对应选择框；无 hint → 三选兜底
pub fn on_detected(app: &AppHandle, id: &str) {
    match take_hint(id) {
        Some(ZmodemHint::Download) => {
            let _ = app.emit(&format!("zmodem-ask-dir-{id}"), ());
        }
        Some(ZmodemHint::Upload) => {
            let _ = app.emit(&format!("zmodem-ask-files-{id}"), ());
        }
        None => {
            let _ = app.emit(&format!("zmodem-request-{id}"), ());
        }
    }
}

/// stash 中多次重发的 ZRQINIT：裁剪到最后一个针起始处（重复帧丢弃）
fn trim_to_last_needle(stash: Vec<u8>) -> Vec<u8> {
    let mut last: Option<usize> = None;
    for n in NEEDLES {
        if stash.len() >= n.len() {
            if let Some(i) = stash
                .windows(n.len())
                .rposition(|w| w == *n)
            {
                last = Some(last.map_or(i, |b: usize| b.max(i)));
            }
        }
    }
    match last {
        Some(i) => stash[i..].to_vec(),
        None => stash,
    }
}

pub type SessionStates = Arc<Mutex<HashMap<String, Arc<Mutex<ZmodemState>>>>>;

#[derive(Default)]
pub struct ZmodemRegistry(pub SessionStates);

pub struct ZmodemState {
    mode: Mode,
}

enum Mode {
    /// 正常透传；保留尾部可能被跨块切断的起始序列片段
    Passive { tail: Vec<u8> },
    /// 检测到 ZRQINIT，等待用户决策；字节暂存
    Pending { stash: Vec<u8> },
    /// 接收中：进站字节送入 rx 泵
    ActiveRx { wire: TxWire },
    /// 发送中：进站字节送入 tx 泵
    ActiveTx { wire: TxWire },
}

type TxWire = mpsc::UnboundedSender<Vec<u8>>;

impl ZmodemState {
    fn new() -> Self {
        Self {
            mode: Mode::Passive { tail: Vec::new() },
        }
    }

    pub fn entry(reg: &ZmodemRegistry, id: &str) -> Arc<Mutex<ZmodemState>> {
        let mut map = reg.0.lock().unwrap();
        map.entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(ZmodemState::new())))
            .clone()
    }

    /// 喂入一段进站字节，返回应透传到终端/缓冲的部分与「检测到传输」标记
    pub fn feed(&mut self, data: &[u8]) -> (Vec<u8>, bool) {
        let old = std::mem::replace(&mut self.mode, Mode::Passive { tail: Vec::new() });
        let (passthrough, detected) = match old {
            Mode::Passive { mut tail } => {
                tail.extend_from_slice(data);
                match find_any_needle(&tail) {
                    Some(i) => {
                        self.mode = Mode::Pending { stash: tail[i..].to_vec() };
                        (tail[..i].to_vec(), true)
                    }
                    None => {
                        let keep = needle_prefix_suffix_len(&tail);
                        let cut = tail.len() - keep;
                        self.mode = Mode::Passive {
                            tail: tail[cut..].to_vec(),
                        };
                        (tail[..cut].to_vec(), false)
                    }
                }
            }
            Mode::Pending { mut stash } => {
                stash.extend_from_slice(data);
                self.mode = Mode::Pending { stash };
                (Vec::new(), false)
            }
            Mode::ActiveRx { wire } => {
                if wire.send(data.to_vec()).is_ok() {
                    self.mode = Mode::ActiveRx { wire };
                    (Vec::new(), false)
                } else {
                    self.mode = Mode::Passive { tail: Vec::new() };
                    (data.to_vec(), false)
                }
            }
            Mode::ActiveTx { wire } => {
                if wire.send(data.to_vec()).is_ok() {
                    self.mode = Mode::ActiveTx { wire };
                    (Vec::new(), false)
                } else {
                    self.mode = Mode::Passive { tail: Vec::new() };
                    (data.to_vec(), false)
                }
            }
        };
        (passthrough, detected)
    }
}

/// 会话关闭：强制复位模式并丢弃通道（泵随之退出）
pub fn reset_on_close(reg: &ZmodemRegistry, id: &str) {
    let entry = ZmodemState::entry(reg, id);
    let mut guard = entry.lock().unwrap();
    guard.mode = Mode::Passive { tail: Vec::new() };
}

/// 最早命中的 needle 位置
fn find_any_needle(buf: &[u8]) -> Option<usize> {
    if buf.len() < 4 {
        return None;
    }
    let mut best: Option<usize> = None;
    for n in NEEDLES {
        if buf.len() < n.len() {
            continue;
        }
        if let Some(i) = buf.windows(n.len()).position(|w| w == *n) {
            best = Some(best.map_or(i, |b| b.min(i)));
        }
    }
    best
}

/// buf 末尾与任一 needle 前缀的最长匹配长度
fn needle_prefix_suffix_len(buf: &[u8]) -> usize {
    let mut keep = 0;
    for n in NEEDLES {
        let max = (n.len() - 1).min(buf.len());
        for l in (1..=max).rev() {
            if buf.ends_with(&n[..l]) {
                keep = keep.max(l);
                break;
            }
        }
    }
    keep
}

#[derive(Serialize, Clone)]
pub struct ZmodemProgress {
    pub kind: String,
    pub name: String,
    pub done: u64,
    pub total: u64,
}

#[derive(Serialize, Clone)]
pub struct ZmodemConflict {
    pub temp: String,
    pub name: String,
    pub dest: String,
    pub size: u64,
}

fn emit_progress(app: &AppHandle, id: &str, kind: &str, name: &str, done: u64, total: u64) {
    let _ = app.emit(
        &format!("zmodem-progress-{id}"),
        ZmodemProgress {
            kind: kind.into(),
            name: name.into(),
            done,
            total,
        },
    );
}

fn emit_conflict(app: &AppHandle, id: &str, name: &str, size: u64, temp: &str, dest: &str) {
    let _ = app.emit(
        &format!("zmodem-conflict-{id}"),
        ZmodemConflict {
            temp: temp.into(),
            name: name.into(),
            dest: dest.into(),
            size,
        },
    );
}

/// 临时接收目录与唯一文件名
fn temp_file_path(app: &AppHandle, id: &str, seq: u32, name: &str) -> PathBufResult {
    let base = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("xtshell-fallback"));
    let dir = base.join("zmodem");
    let _ = std::fs::create_dir_all(&dir);
    let basename = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    Ok(dir.join(format!("{id}-{seq}-{nanos}-{basename}")))
}

type PathBufResult = Result<std::path::PathBuf, ()>;

async fn write_wire(mgr: &SharedManager, id: &str, bytes: &[u8]) -> Result<(), AppError> {
    mgr.lock()
        .await
        .send_input(id, bytes)
        .await
        .map_err(|e| AppError::Connection(e.to_string()))
}

/// 临时文件搬移：rename 优先，跨盘退化为 copy+delete
async fn move_file(temp: &str, dest: &str) -> Result<(), String> {
    if tokio::fs::rename(temp, dest).await.is_ok() {
        return Ok(());
    }
    tokio::fs::copy(temp, dest)
        .await
        .map_err(|e| format!("{temp} -> {dest}: {e}"))?;
    let _ = tokio::fs::remove_file(temp).await;
    Ok(())
}

/// 用户选择保存目录后启动 Receiver 泵：文件先写临时文件，完成后落到选定目录
pub fn start_receiver(
    app: AppHandle,
    mgr: SharedManager,
    reg: &ZmodemRegistry,
    id: &SessionId,
    dest_dir: Option<String>,
) -> Result<(), AppError> {
    let entry = ZmodemState::entry(reg, id);
    let stash = {
        let mut guard = entry.lock().unwrap();
        match &mut guard.mode {
            Mode::Pending { stash } => std::mem::take(stash),
            _ => return Err(AppError::Connection("zmodem: no pending transfer".into())),
        }
    };
    let stash = trim_to_last_needle(stash);
    let (wire_tx, wire_rx) = mpsc::unbounded_channel();
    {
        let mut guard = entry.lock().unwrap();
        guard.mode = Mode::ActiveRx { wire: wire_tx };
    }

    let id2 = id.clone();
    tauri::async_runtime::spawn(async move {
        run_rx(app, mgr, entry, id2, wire_rx, stash, dest_dir).await;
    });
    Ok(())
}

/// 接收泵：驱动 Receiver 状态机直到会话结束。
/// 自动接收：文件先写临时文件，完成后 rename 到选定目录（冲突时交前端裁决）。
async fn run_rx(
    app: AppHandle,
    mgr: SharedManager,
    entry: Arc<Mutex<ZmodemState>>,
    id: String,
    mut wire_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    stash: Vec<u8>,
    dest_dir: Option<String>,
) {
    let done = PumpDone {
        entry,
        app: app.clone(),
        id: id.clone(),
    };
    let mut receiver = match Receiver::with_flow_control(0, true) {
        Ok(r) => r,
        Err(_) => {
            emit_progress(&app, &id, "error", "receiver init failed", 0, 0);
            finish_pump(done, wire_rx).await;
            return;
        }
    };

    let mut input: Vec<u8> = stash;
    let mut file: Option<tokio::fs::File> = None;
    let mut file_name = String::new();
    let mut temp_path = String::new();
    let mut file_done: u64 = 0;
    let mut file_total: u64 = 0;
    let mut file_seq: u32 = 0;
    let mut finishing = false;

    loop {
        // 结束阶段：先冲刷排队的协议回应帧（ZFIN 等），再退出
        if finishing {
            match receiver.poll() {
                Action::WriteWire(bytes) => {
                    let owned = bytes.to_vec();
                    if write_wire(&mgr, &id, &owned).await.is_ok() {
                        receiver.wire_written(owned.len());
                        continue;
                    }
                }
                _ => {}
            }
            break;
        }
        let mut progressed = false;
        if !input.is_empty() {
            match receiver.submit_wire(&input) {
                Ok(n) if n > 0 => {
                    input.drain(..n);
                    progressed = true;
                }
                _ => {}
            }
        }

        match receiver.poll() {
            Action::WriteWire(bytes) => {
                let owned = bytes.to_vec();
                if write_wire(&mgr, &id, &owned).await.is_ok() {
                    receiver.wire_written(owned.len());
                    progressed = true;
                } else {
                    let _ = receiver.abort();
                }
            }
            Action::WriteFile(bytes) => {
                let owned = bytes.to_vec();
                match file.as_mut() {
                    Some(f) => {
                        if f.write_all(&owned).await.is_ok() {
                            file_done += owned.len() as u64;
                            let _ = receiver.file_written(owned.len());
                            emit_progress(&app, &id, "progress", &file_name, file_done, file_total);
                            progressed = true;
                        } else {
                            let _ = receiver.abort();
                        }
                    }
                    None => {
                        // 理论不达（自动接收）：尽快打开
                        let _ = receiver.file_written(owned.len());
                        progressed = true;
                    }
                }
            }
            Action::Event(event) => match event {
                Event::FileStarted(info) => {
                    file_name = String::from_utf8_lossy(info.name).into_owned();
                    file_done = 0;
                    file_total = info.size.map(|s| s.get() as u64).unwrap_or(0);
                    file_seq += 1;
                    match temp_file_path(&app, &id, file_seq, &file_name) {
                        Ok(path) => match tokio::fs::File::create(&path).await {
                            Ok(f) => {
                                temp_path = path.to_string_lossy().into_owned();
                                file = Some(f);
                                let _ = receiver.accept_file_at(0);
                            }
                            Err(e) => {
                                emit_progress(
                                    &app,
                                    &id,
                                    "error",
                                    &format!("create temp: {e}"),
                                    0,
                                    0,
                                );
                                let _ = receiver.skip_file();
                            }
                        },
                        Err(_) => {
                            let _ = receiver.skip_file();
                        }
                    }
                    emit_progress(&app, &id, "start", &file_name, 0, file_total);
                    progressed = true;
                }
                Event::FileCompleted => {
                    file = None;
                    progressed = true;
                    // 落盘：有目录 → rename；同名冲突 → 交前端裁决
                    match &dest_dir {
                        Some(dir) => {
                            let base = std::path::Path::new(&file_name)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("file")
                                .to_string();
                            let dest = std::path::Path::new(dir).join(&base);
                            let dest_str = dest.to_string_lossy().into_owned();
                            if dest.exists() {
                                emit_conflict(&app, &id, &base, file_total, &temp_path, &dest_str);
                            } else {
                                match move_file(&temp_path, &dest_str).await {
                                    Ok(()) => {
                                        emit_progress(
                                            &app, &id, "saved", &dest_str, file_done, file_total,
                                        );
                                    }
                                    Err(e) => {
                                        emit_progress(&app, &id, "error", &e, 0, 0);
                                    }
                                }
                            }
                        }
                        None => {
                            emit_progress(&app, &id, "unsaved", &temp_path, file_done, file_total)
                        }
                    }
                    // 临时文件责任已移交（moved 或 conflict 命令处理），泵不再负责清理
                    temp_path.clear();
                }
                Event::SessionCompleted => {
                    emit_progress(&app, &id, "complete", "", 0, 0);
                    finishing = true;
                }
                Event::Aborted => {
                    emit_progress(&app, &id, "aborted", "", 0, 0);
                    finishing = true;
                }
                _ => {}
            },
            Action::Idle => {}
            Action::ReadFile { .. } => unreachable!("receiver never reads files"),
            _ => {}
        }

        // 无进展：等 wire 输入（超时驱动协议重排队）
        if !progressed {
            match tokio::time::timeout(Duration::from_secs(1), wire_rx.recv()).await {
                Ok(Some(chunk)) => input.extend_from_slice(&chunk),
                Ok(None) => break,
                Err(_) => {
                    let _ = receiver.timeout();
                }
            }
        }
    }

    // 泵退出：未接收完整的临时文件删除（数据不完整，不留垃圾）
    if !temp_path.is_empty() {
        let _ = tokio::fs::remove_file(&temp_path).await;
    }

    finish_pump(done, wire_rx).await;
}

struct PumpDone {
    entry: Arc<Mutex<ZmodemState>>,
    app: AppHandle,
    id: String,
}

/// 泵结束后：模式复位，残余 wire 输入透传回终端
async fn finish_pump(done: PumpDone, mut wire_rx: mpsc::UnboundedReceiver<Vec<u8>>) {
    {
        let mut guard = done.entry.lock().unwrap();
        guard.mode = Mode::Passive { tail: Vec::new() };
    }
    let mut leftover = Vec::new();
    while let Ok(chunk) = wire_rx.try_recv() {
        leftover.extend_from_slice(&chunk);
    }
    if !leftover.is_empty() {
        let _ = done.app.emit(&format!("session-output-{}", done.id), leftover);
    }
}

/// 用户选择「发送」：启动 Sender 泵
pub fn start_sender(
    app: AppHandle,
    mgr: SharedManager,
    reg: &ZmodemRegistry,
    id: &SessionId,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let entry = ZmodemState::entry(reg, id);
    let stash = {
        let mut guard = entry.lock().unwrap();
        match &mut guard.mode {
            Mode::Pending { stash } => std::mem::take(stash),
            _ => return Err(AppError::Connection("zmodem: no pending transfer".into())),
        }
    };
    let stash = trim_to_last_needle(stash);
    let (wire_tx, wire_rx) = mpsc::unbounded_channel();
    {
        let mut guard = entry.lock().unwrap();
        guard.mode = Mode::ActiveTx { wire: wire_tx };
    }

    let id2 = id.clone();
    tauri::async_runtime::spawn(async move {
        run_tx(app, mgr, entry, id2, wire_rx, stash, paths).await;
    });
    Ok(())
}

/// 发送泵：驱动 Sender 状态机，逐个发送文件
async fn run_tx(
    app: AppHandle,
    mgr: SharedManager,
    entry: Arc<Mutex<ZmodemState>>,
    id: String,
    mut wire_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    stash: Vec<u8>,
    paths: Vec<String>,
) {
    let done = PumpDone {
        entry,
        app: app.clone(),
        id: id.clone(),
    };
    let mut sender = match Sender::new() {
        Ok(s) => s,
        Err(_) => {
            emit_progress(&app, &id, "error", "sender init failed", 0, 0);
            finish_pump(done, wire_rx).await;
            return;
        }
    };
    sender.set_streaming_window(usize::MAX);

    let mut input = stash;
    let mut paths_iter = paths.into_iter();
    let mut finishing = false;

    // 起始：发送第一个文件
    let mut current: Option<(tokio::fs::File, String, u64)> = match paths_iter.next() {
        Some(path) => match open_for_send(&path).await {
            Ok((f, name, size)) => match sender.start_file(FileInfo::new(
                name.as_bytes(),
                Some(Position::new(size as u32)),
            )) {
                Ok(()) => {
                    emit_progress(&app, &id, "start", &name, 0, size);
                    Some((f, name, size))
                }
                Err(e) => {
                    emit_progress(&app, &id, "error", &format!("{name}: {e}"), 0, 0);
                    let _ = sender.abort();
                    None
                }
            },
            Err(e) => {
                emit_progress(&app, &id, "error", &e, 0, 0);
                let _ = sender.abort();
                None
            }
        },
        None => None,
    };

    loop {
        // 结束阶段：先冲刷排队的协议回应帧，再退出
        if finishing {
            match sender.poll() {
                Action::WriteWire(bytes) => {
                    let owned = bytes.to_vec();
                    if write_wire(&mgr, &id, &owned).await.is_ok() {
                        sender.wire_written(owned.len());
                        continue;
                    }
                }
                _ => {}
            }
            break;
        }
        let mut progressed = false;
        if !input.is_empty() {
            match sender.submit_wire(&input) {
                Ok(n) if n > 0 => {
                    input.drain(..n);
                    progressed = true;
                }
                _ => {}
            }
        }

        match sender.poll() {
            Action::WriteWire(bytes) => {
                let owned = bytes.to_vec();
                if write_wire(&mgr, &id, &owned).await.is_ok() {
                    sender.wire_written(owned.len());
                    progressed = true;
                } else {
                    let _ = sender.abort();
                }
            }
            Action::ReadFile { offset, max_len } => {
                if let Some((f, name, size)) = current.as_mut() {
                    if f.seek(std::io::SeekFrom::Start(offset.get() as u64))
                        .await
                        .is_err()
                    {
                        let _ = sender.abort();
                        continue;
                    }
                    let mut buf = vec![0u8; max_len.min(4096)];
                    match f.read(&mut buf).await {
                        Ok(0) => {
                            // EOF：文件已读完（不应发生，防御）
                            let _ = sender.submit_file(&[]);
                        }
                        Ok(n) => {
                            let done = offset.get() as u64 + n as u64;
                            emit_progress(&app, &id, "progress", name, done, *size);
                            if sender.submit_file(&buf[..n]).is_err() {
                                let _ = sender.abort();
                            }
                        }
                        Err(e) => {
                            emit_progress(&app, &id, "error", &format!("{name}: {e}"), 0, 0);
                            let _ = sender.abort();
                        }
                    }
                    progressed = true;
                } else {
                    let _ = sender.abort();
                }
            }
            Action::Event(event) => match event {
                Event::FileCompleted => {
                    if let Some((_, name, _)) = current.take() {
                        emit_progress(&app, &id, "done", &name, 0, 0);
                    }
                    progressed = true;
                    // 发送下一个文件或结束会话
                    current = match paths_iter.next() {
                        Some(path) => match open_for_send(&path).await {
                            Ok((f, name, size)) => {
                                match sender.start_file(FileInfo::new(
                                    name.as_bytes(),
                                    Some(Position::new(size as u32)),
                                )) {
                                    Ok(()) => {
                                        emit_progress(&app, &id, "start", &name, 0, size);
                                        Some((f, name, size))
                                    }
                                    Err(e) => {
                                        emit_progress(&app, &id, "error", &format!("{name}: {e}"), 0, 0);
                                        let _ = sender.abort();
                                        None
                                    }
                                }
                            }
                            Err(e) => {
                                emit_progress(&app, &id, "error", &e, 0, 0);
                                let _ = sender.abort();
                                None
                            }
                        },
                        None => {
                            if sender.finish().is_err() {
                                let _ = sender.abort();
                            }
                            None
                        }
                    };
                }
                Event::SessionCompleted => {
                    emit_progress(&app, &id, "complete", "", 0, 0);
                    finishing = true;
                }
                Event::Aborted => {
                    emit_progress(&app, &id, "aborted", "", 0, 0);
                    finishing = true;
                }
                _ => {}
            },
            Action::WriteFile(_) => unreachable!("sender never writes files"),
            Action::Idle => {}
            _ => {}
        }

        // 无进展：等 wire 输入（超时驱动协议重排队）
        if !progressed {
            match tokio::time::timeout(Duration::from_secs(1), wire_rx.recv()).await {
                Ok(Some(chunk)) => input.extend_from_slice(&chunk),
                Ok(None) => break,
                Err(_) => {
                    let _ = sender.timeout();
                }
            }
        }
    }

    finish_pump(done, wire_rx).await;
}

async fn open_for_send(path: &str) -> Result<(tokio::fs::File, String, u64), String> {
    let f = tokio::fs::File::open(path)
        .await
        .map_err(|e| format!("{path}: {e}"))?;
    let meta = f.metadata().await.map_err(|e| format!("{path}: {e}"))?;
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();
    Ok((f, name, meta.len()))
}

/// 用户取消：用暂存字节驱动一次 abort，让远端干净退出
pub async fn cancel_transfer(
    app: AppHandle,
    mgr: SharedManager,
    reg: &ZmodemRegistry,
    id: &SessionId,
) {
    let entry = ZmodemState::entry(reg, id);
    let stash = {
        let mut guard = entry.lock().unwrap();
        match &mut guard.mode {
            Mode::Pending { stash } => std::mem::take(stash),
            _ => return,
        }
    };
    emit_progress(&app, id, "cancelled", "", 0, 0);

    // 主动发送 CAN 序列让远端 zmodem 退出
    if let Ok(mut receiver) = Receiver::new() {
        let mut input = stash;
        for _ in 0..32 {
            if !input.is_empty() {
                let _ = receiver.submit_wire(&input);
                input.clear();
            }
            match receiver.poll() {
                Action::WriteWire(bytes) => {
                    let owned = bytes.to_vec();
                    if write_wire(&mgr, id, &owned).await.is_ok() {
                        receiver.wire_written(owned.len());
                    }
                }
                Action::Idle => break,
                _ => {}
            }
        }
        let _ = receiver.abort();
        for _ in 0..8 {
            match receiver.poll() {
                Action::WriteWire(bytes) => {
                    let owned = bytes.to_vec();
                    if write_wire(&mgr, id, &owned).await.is_ok() {
                        receiver.wire_written(owned.len());
                    }
                }
                _ => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[test]
    fn feed_passthrough_normal_data() {
        let mut s = ZmodemState::new();
        let (p, d) = s.feed(b"hello world\r\n");
        assert_eq!(p, b"hello world\r\n");
        assert!(!d);
    }

    #[test]
    fn feed_detects_sz_zrqinit_across_chunks() {
        let mut s = ZmodemState::new();
        // "hello **" 末尾 "**" 是 hex 针前缀，保留不透传
        let (p, d) = s.feed(b"hello **");
        assert_eq!(p, b"hello ");
        assert!(!d);
        // 续上 "\x18B00"：二进制针在 index 1 命中，先前保留的 "*" 先透传
        let (p, d) = s.feed(b"\x18B00rz");
        assert_eq!(p, b"*");
        assert!(d);
    }

    #[test]
    fn feed_detects_rz_hex_zrqinit() {
        // 真机捕获：rz 输出 "**B0100000023be50"（无 \x18 的 hex 形式）
        let mut s = ZmodemState::new();
        let (p, d) = s.feed(b"rz waiting to receive.**B0100000023be50");
        assert_eq!(p, b"rz waiting to receive.");
        assert!(d);
    }

    #[test]
    fn feed_detects_rz_binary_variant() {
        let mut s = ZmodemState::new();
        let (p, d) = s.feed(b"**\x18B0100000023be50");
        assert_eq!(p, b"*");
        assert!(d);
    }

    #[test]
    fn pending_stashes_until_decision() {
        let mut s = ZmodemState::new();
        let _ = s.feed(b"*\x18B00");
        let (p, _) = s.feed(b"more bytes");
        assert!(p.is_empty());
    }

    #[test]
    fn note_input_classifies_lines() {
        let id = "hint-test-1";
        let map = INPUT_LINES.get_or_init(|| Mutex::new(HashMap::new()));
        map.lock().unwrap().insert(
            id.to_string(),
            LineState { buf: String::new(), hint: None },
        );
        note_input(id, "sz /etc/hostname\r");
        assert_eq!(take_hint(id), Some(ZmodemHint::Download));
        assert_eq!(take_hint(id), None); // 用后即清

        map.lock().unwrap().insert(
            id.to_string(),
            LineState { buf: String::new(), hint: None },
        );
        note_input(id, "rz\r");
        assert_eq!(take_hint(id), Some(ZmodemHint::Upload));

        map.lock().unwrap().insert(
            id.to_string(),
            LineState { buf: String::new(), hint: None },
        );
        note_input(id, "ls -la\r");
        assert_eq!(take_hint(id), None);
    }

    #[test]
    fn note_input_backspace_and_ctrl_u() {
        let id = "hint-edit-test";
        let map = INPUT_LINES.get_or_init(|| Mutex::new(HashMap::new()));
        map.lock().unwrap().insert(
            id.to_string(),
            LineState { buf: String::new(), hint: None },
        );
        note_input(id, "rzz\x7f\r");
        assert_eq!(take_hint(id), Some(ZmodemHint::Upload));
        note_input(id, "ls\x15sz\r");
        assert_eq!(take_hint(id), Some(ZmodemHint::Download));
    }

    #[test]
    fn trim_to_last_needle_drops_repeats() {
        let zrq = b"*B00".to_vec();
        let mut stash = zrq.clone();
        stash.extend_from_slice(b"banner");
        stash.extend_from_slice(&zrq);
        stash.extend_from_slice(b"tail");
        let t = trim_to_last_needle(stash);
        // 应从最后一个针开始，重复与 banner 丢弃
        assert!(t.starts_with(b"*B00"));
        assert_eq!(t, [zrq.as_slice(), b"tail".as_slice()].concat());
    }

    #[test]
    fn needle_prefix_suffix() {
        assert_eq!(needle_prefix_suffix_len(b"abc*\x18"), 2);
        assert_eq!(needle_prefix_suffix_len(b"abc"), 0);
        assert_eq!(needle_prefix_suffix_len(b"abc*"), 1);
        // hex 针跨块："**B0100000023be50" 切在 "…B0100" 后 → 保留 7 字节
        assert_eq!(needle_prefix_suffix_len(b"abc**B0100"), 7);
    }

    /// 本地 Sender↔Receiver 回环对打：验证状态机用法与流式收发正确性
    #[tokio::test]
    async fn sender_receiver_loopback() {
        let dir = std::env::temp_dir().join(format!(
            "xtshell-zmodem-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("hello.bin");
        let dst = dir.join("hello_out.bin");

        // 生成 100KB 伪随机内容
        let content: Vec<u8> = (0..100 * 1024)
            .map(|i| ((i as u64 * 2654435761u64 % 256) as u8))
            .collect();
        std::fs::write(&src, &content).unwrap();

        let (s2r_tx, mut s2r_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (r2s_tx, mut r2s_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        let send_src = src.clone();
        let send_task = tokio::spawn(async move {
            let src = send_src;
            let mut sender = Sender::new().unwrap();
            sender.set_streaming_window(usize::MAX);
            let mut f = tokio::fs::File::open(&src).await.unwrap();
            let size = f.metadata().await.unwrap().len() as u32;
            let name = "hello.bin".to_string();
            sender
                .start_file(FileInfo::new(name.as_bytes(), Some(Position::new(size))))
                .unwrap();
            let mut input: Vec<u8> = Vec::new();
            let mut finishing = false;
            loop {
                if finishing {
                    match sender.poll() {
                        Action::WriteWire(b) => {
                            let len = b.len();
                            if s2r_tx.send(b.to_vec()).is_ok() {
                                sender.wire_written(len);
                                continue;
                            }
                        }
                        _ => {}
                    }
                    break;
                }
                let mut progressed = false;
                if !input.is_empty() {
                    match sender.submit_wire(&input) {
                        Ok(n) if n > 0 => {
                            input.drain(..n);
                            progressed = true;
                        }
                        _ => {}
                    }
                }
                match sender.poll() {
                    Action::WriteWire(b) => {
                        let len = b.len();
                        s2r_tx.send(b.to_vec()).unwrap();
                        sender.wire_written(len);
                        progressed = true;
                    }
                    Action::ReadFile { offset, max_len } => {
                        use tokio::io::AsyncSeekExt;
                        f.seek(std::io::SeekFrom::Start(offset.get() as u64))
                            .await
                            .unwrap();
                        let mut buf = vec![0u8; max_len.min(4096)];
                        let n = f.read(&mut buf).await.unwrap();
                        sender.submit_file(&buf[..n]).unwrap();
                        progressed = true;
                    }
                    Action::Event(Event::FileCompleted) => {
                        sender.finish().unwrap();
                        progressed = true;
                    }
                    Action::Event(Event::SessionCompleted) => finishing = true,
                    Action::Event(Event::Aborted) => panic!("sender aborted"),
                    Action::Idle => {}
                    _ => {}
                }
                if !progressed {
                    match tokio::time::timeout(Duration::from_millis(100), r2s_rx.recv()).await {
                        Ok(Some(c)) => input.extend_from_slice(&c),
                        Ok(None) => {
                            let _ = sender.abort();
                        }
                        Err(_) => {
                            let _ = sender.timeout();
                        }
                    }
                }
            }
        });

        let recv_dst = dst.clone();
        let recv_task = tokio::spawn(async move {
            let mut receiver = Receiver::with_flow_control(0, true).unwrap();
            let dst = recv_dst;
            let mut out = tokio::fs::File::create(&dst).await.unwrap();
            let mut input: Vec<u8> = Vec::new();
            let mut finishing = false;
            loop {
                if finishing {
                    match receiver.poll() {
                        Action::WriteWire(b) => {
                            let len = b.len();
                            if r2s_tx.send(b.to_vec()).is_ok() {
                                receiver.wire_written(len);
                                continue;
                            }
                        }
                        _ => {}
                    }
                    break;
                }
                let mut progressed = false;
                if !input.is_empty() {
                    match receiver.submit_wire(&input) {
                        Ok(n) if n > 0 => {
                            input.drain(..n);
                            progressed = true;
                        }
                        _ => {}
                    }
                }
                match receiver.poll() {
                    Action::WriteWire(b) => {
                        let len = b.len();
                        r2s_tx.send(b.to_vec()).unwrap();
                        receiver.wire_written(len);
                        progressed = true;
                    }
                    Action::WriteFile(b) => {
                        let owned = b.to_vec();
                        out.write_all(&owned).await.unwrap();
                        receiver.file_written(owned.len()).unwrap();
                        progressed = true;
                    }
                    Action::Event(Event::SessionCompleted) => finishing = true,
                    Action::Event(Event::Aborted) => panic!("receiver aborted"),
                    Action::Idle => {}
                    _ => {}
                }
                if !progressed {
                    match tokio::time::timeout(Duration::from_millis(100), s2r_rx.recv()).await {
                        Ok(Some(c)) => input.extend_from_slice(&c),
                        Ok(None) => {
                            let _ = receiver.abort();
                        }
                        Err(_) => {
                            let _ = receiver.timeout();
                        }
                    }
                }
            }
        });

        let (r1, r2) = tokio::join!(send_task, recv_task);
        r1.unwrap();
        r2.unwrap();

        assert_eq!(std::fs::read(&src).unwrap(), std::fs::read(&dst).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }
}
