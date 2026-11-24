use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};

use crate::error::{AppError, AppResult};
use crate::protocol::{ClosedSignal, OutputSink, TerminalStream};

/// 本地 Shell 会话：用系统伪终端（Windows ConPTY / Unix PTY）运行本机命令行，
/// 终端里看到的行为与直接打开 cmd.exe / bash 一致。
///
/// 与 SSH/Telnet 相同：读任务自持 reader 并回调 OutputSink；
/// 写入走独立 writer，读任务不持写锁。
pub struct LocalPtyStream {
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

/// 把一段字符串按空白拆成 token：双/单引号可包裹含空格的片段，
/// 反斜杠原样保留（Windows 路径 `C:\Windows\System32\cmd.exe` 不能被当成转义）。
fn tokenize(input: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in input.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    cur.push(ch);
                }
            }
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        parts.push(std::mem::take(&mut cur));
                    }
                }
                c => cur.push(c),
            },
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

/// 拆参数串：空白分隔，双/单引号可包裹含空格的参数。
pub fn split_args(args: &str) -> Vec<String> {
    tokenize(args)
}

/// 清理程序路径：去首尾空白与一对包裹引号（文件选择框返回的路径不带引号，
/// 但手工粘贴时常会带）。
pub fn normalize_program(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let quoted = (bytes[0] == b'"' && bytes[trimmed.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[trimmed.len() - 1] == b'\'');
        if quoted {
            return trimmed[1..trimmed.len() - 1].trim().to_string();
        }
    }
    trimmed.to_string()
}

/// 未配置本地 Shell 时的系统默认：Windows 取 COMSPEC（cmd.exe），Unix 取 $SHELL。
pub fn default_shell() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "cmd.exe".into())
    } else {
        std::env::var("SHELL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "/bin/sh".into())
    }
}

fn home_dir() -> Option<std::ffi::OsString> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key).filter(|v| !v.is_empty())
}

fn pty_size(cols: u32, rows: u32) -> PtySize {
    PtySize {
        cols: cols.clamp(1, u16::MAX as u32) as u16,
        rows: rows.clamp(1, u16::MAX as u32) as u16,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// 关闭信号只允许触发一次：读端 EOF 与子进程退出可能同时发生。
type ClosedSlot = Arc<Mutex<Option<ClosedSignal>>>;

fn fire_closed(slot: &ClosedSlot) {
    let signal = slot.lock().ok().and_then(|mut guard| guard.take());
    if let Some(f) = signal {
        f();
    }
}

impl LocalPtyStream {
    pub fn open(
        shell: Option<&str>,
        shell_args: Option<&str>,
        cols: u32,
        rows: u32,
        on_output: OutputSink,
        on_closed: ClosedSignal,
    ) -> AppResult<Self> {
        let explicit = shell.map(normalize_program).filter(|s| !s.is_empty());
        // 路径留空 = 系统默认 Shell；此时参数忽略（前端也会把参数框置灰）
        let args: Vec<String> = match explicit.as_deref() {
            Some(_) => shell_args.map(split_args).unwrap_or_default(),
            None => Vec::new(),
        };
        let program = explicit.unwrap_or_else(default_shell);
        if program.is_empty() {
            return Err(AppError::Connection("未配置 Shell".into()));
        }
        let pair = native_pty_system()
            .openpty(pty_size(cols, rows))
            .map_err(|e| AppError::Connection(format!("无法创建本地终端：{e}")))?;

        let mut cmd = CommandBuilder::new(&program);
        for arg in &args {
            cmd.arg(arg);
        }
        // GUI 进程的当前目录不可控（可能是 system32），本地 Shell 从用户主目录起步
        if let Some(home) = home_dir() {
            cmd.cwd(home);
        }
        // 让 ncurses/readline 类程序按 xterm 输出（主要影响 Unix）
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        // 先拿到 reader/writer 再启动子进程：任一失败时不会留下无人看管的 shell
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| AppError::Connection(format!("读取本地终端失败：{e}")))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| AppError::Connection(format!("写入本地终端失败：{e}")))?;

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| AppError::Connection(format!("无法启动本地 Shell「{program}」：{e}")))?;
        // slave 只用于 spawn；持有它会挡住 master 读端的 EOF
        drop(pair.slave);
        let mut killer = child.clone_killer();
        // 单独回收子进程，避免 shell 自行退出后留下僵尸进程；
        // 读任务结束（EOF）才是会话关闭信号。
        let closed_slot: ClosedSlot = Arc::new(Mutex::new(Some(on_closed)));
        let read_closed = closed_slot.clone();
        // Windows ConPTY 在 shell 退出后不会立刻让输出管道 EOF（伪控制台句柄仍存活），
        // 因此额外用子进程退出作为关闭信号；Unix 侧仍以读端 EOF 为准。
        #[cfg(windows)]
        let wait_closed = closed_slot.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("xtshell-local-wait".into())
            .spawn(move || {
                let _ = child.wait();
                #[cfg(windows)]
                fire_closed(&wait_closed);
            })
        {
            let _ = killer.kill();
            return Err(AppError::Connection(format!(
                "启动本地终端回收线程失败：{e}"
            )));
        }

        if let Err(e) = std::thread::Builder::new()
            .name("xtshell-local-read".into())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => on_output(&buf[..n]),
                    }
                }
                fire_closed(&read_closed);
            })
        {
            let _ = killer.kill();
            return Err(AppError::Connection(format!(
                "启动本地终端读取线程失败：{e}"
            )));
        }

        Ok(LocalPtyStream {
            master: pair.master,
            writer: Arc::new(Mutex::new(Some(writer))),
            killer,
        })
    }
}

#[async_trait]
impl TerminalStream for LocalPtyStream {
    async fn write(&mut self, data: &[u8]) -> AppResult<()> {
        let writer = self.writer.clone();
        let owned = data.to_vec();
        // 阻塞式写放到 blocking 线程池：ConPTY 输入管道写满时不卡住 tokio worker
        tokio::task::spawn_blocking(move || {
            let mut guard = writer
                .lock()
                .map_err(|_| AppError::Connection("本地终端写入锁失效".into()))?;
            let w = guard
                .as_mut()
                .ok_or_else(|| AppError::Connection("本地终端已关闭".into()))?;
            w.write_all(&owned).map_err(AppError::Io)?;
            w.flush().map_err(AppError::Io)
        })
        .await
        .map_err(|e| AppError::Connection(format!("本地终端写入失败：{e}")))?
    }

    async fn resize(&mut self, cols: u32, rows: u32) -> AppResult<()> {
        self.master
            .resize(pty_size(cols, rows))
            .map_err(|e| AppError::Connection(format!("调整本地终端尺寸失败：{e}")))
    }

    async fn close(&mut self) -> AppResult<()> {
        if let Ok(mut guard) = self.writer.lock() {
            guard.take();
        }
        let _ = self.killer.kill();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_arguments() {
        assert_eq!(split_args("-NoLogo -NoExit"), ["-NoLogo", "-NoExit"]);
    }

    #[test]
    fn keeps_quoted_argument_with_spaces_and_backslashes() {
        assert_eq!(
            split_args(r#"-File "C:\Program Files\a b.ps1""#),
            ["-File", r"C:\Program Files\a b.ps1"]
        );
    }

    #[test]
    fn tolerates_extra_whitespace_in_args() {
        assert_eq!(split_args("  -i   -l  "), ["-i", "-l"]);
    }

    #[test]
    fn empty_args_yield_empty_vec() {
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn normalizes_program_path() {
        assert_eq!(
            normalize_program(r#"  "C:\Program Files\Git\bin\bash.exe"  "#),
            r"C:\Program Files\Git\bin\bash.exe"
        );
        assert_eq!(normalize_program("cmd.exe"), "cmd.exe");
        assert_eq!(normalize_program("   "), "");
        // 只有一侧引号时按普通路径处理
        assert_eq!(
            normalize_program("\"C:\\Program Files\\a.exe"),
            "\"C:\\Program Files\\a.exe"
        );
    }

    #[test]
    fn default_shell_is_non_empty() {
        assert!(!default_shell().trim().is_empty());
    }

    /// 端到端验证：默认 shell 在 PTY 里能收到输入、回显输出，退出时触发 closed。
    #[tokio::test]
    async fn local_pty_runs_shell_and_reports_closed() {
        let out: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let out2 = out.clone();
        let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let closed2 = closed.clone();
        let mut stream = LocalPtyStream::open(
            None,
            None,
            80,
            24,
            Arc::new(move |data: &[u8]| out2.lock().unwrap().extend_from_slice(data)),
            Box::new(move || {
                closed2.store(true, std::sync::atomic::Ordering::SeqCst);
            }),
        )
        .expect("open local pty");

        stream
            .write(b"echo xtshell-ok\r\n")
            .await
            .expect("write to local pty");
        let seen = wait_for_output(&mut stream, &out, "xtshell-ok").await;
        assert!(
            seen,
            "local pty output missing echo: {:?}",
            String::from_utf8_lossy(&out.lock().unwrap())
        );

        stream.write(b"exit\r\n").await.expect("write exit");
        let mut exited = false;
        for _ in 0..250 {
            if closed.load(std::sync::atomic::Ordering::SeqCst) {
                exited = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(exited, "local pty did not report closed after exit");
        let _ = stream.close().await;
    }

    /// 参数串要真的下发到 shell：Unix 用 `sh -c`，Windows 用 `cmd /c`。
    #[tokio::test]
    async fn local_pty_passes_args_to_shell() {
        let out: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let out2 = out.clone();
        #[cfg(windows)]
        let (shell, args) = (default_shell(), "/c echo xtshell-args".to_string());
        #[cfg(not(windows))]
        let (shell, args) = (
            "/bin/sh".to_string(),
            "-c \"echo xtshell-args\"".to_string(),
        );
        let mut stream = LocalPtyStream::open(
            Some(&shell),
            Some(&args),
            80,
            24,
            Arc::new(move |data: &[u8]| out2.lock().unwrap().extend_from_slice(data)),
            Box::new(|| {}),
        )
        .expect("open local pty with args");
        assert!(
            wait_for_output(&mut stream, &out, "xtshell-args").await,
            "args were not passed to shell: {:?}",
            String::from_utf8_lossy(&out.lock().unwrap())
        );
        let _ = stream.close().await;
    }

    /// 等 `marker` 出现；Windows ConPTY 的 DSR（ESC[6n）需要终端回复，测试里回 1;1。
    async fn wait_for_output(
        stream: &mut LocalPtyStream,
        out: &Arc<Mutex<Vec<u8>>>,
        marker: &str,
    ) -> bool {
        let mut answered_dsr = false;
        for _ in 0..250 {
            let snapshot = out.lock().unwrap().clone();
            if !answered_dsr && snapshot.windows(4).any(|w| w == b"\x1b[6n") {
                stream.write(b"\x1b[1;1R").await.expect("answer dsr");
                answered_dsr = true;
            }
            if String::from_utf8_lossy(&snapshot).contains(marker) {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        false
    }
}
