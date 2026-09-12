use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

use crate::error::{AppError, AppResult};
use crate::protocol::{ClosedSignal, OutputSink, TerminalStream};

use std::sync::Mutex;

enum SerialWriter {
    Tokio(tokio::io::WriteHalf<tokio_serial::SerialStream>),
    Std(Arc<Mutex<std::fs::File>>),
}

pub struct SerialConn {
    writer: Option<SerialWriter>,
    close_notify: Arc<Notify>,
    win_close_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl SerialConn {
    pub fn open(
        path: &str,
        baud: u32,
        data_bits: Option<u8>,
        stop_bits: Option<u8>,
        parity: Option<&str>,
        on_output: OutputSink,
        on_closed: ClosedSignal,
    ) -> AppResult<Self> {
        let data_bits = match data_bits {
            Some(5) => DataBits::Five,
            Some(6) => DataBits::Six,
            Some(7) => DataBits::Seven,
            _ => DataBits::Eight,
        };
        let stop_bits = if stop_bits == Some(2) {
            StopBits::Two
        } else {
            StopBits::One
        };
        let parity = match parity {
            Some("Odd") => Parity::Odd,
            Some("Even") => Parity::Even,
            Some(p @ ("Mark" | "Space")) => {
                return Err(AppError::Connection(format!(
                    "parity '{p}' is not supported by the serial library (only None/Odd/Even)"
                )));
            }
            _ => Parity::None,
        };
        let close_notify = Arc::new(Notify::new());
        let win_close_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

        match tokio_serial::new(path, baud)
            .data_bits(data_bits)
            .stop_bits(stop_bits)
            .parity(parity)
            .flow_control(FlowControl::None)
            .open_native_async()
        {
            Ok(stream) => {
                let (mut read_half, write_half) = tokio::io::split(stream);
                let reader_close = close_notify.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    loop {
                        tokio::select! {
                            _ = reader_close.notified() => {
                                on_closed();
                                return;
                            }
                            res = read_half.read(&mut buf) => {
                                match res {
                                    Ok(0) | Err(_) => {
                                        on_closed();
                                        return;
                                    }
                                    Ok(n) => on_output(&buf[..n]),
                                }
                            }
                        }
                    }
                });
                Ok(SerialConn {
                    writer: Some(SerialWriter::Tokio(write_half)),
                    close_notify,
                    win_close_flag,
                })
            }
            Err(primary_err) => {
                match win_fallback::open_blocking(path, baud, win_close_flag.clone(), on_output.clone(), on_closed) {
                    Ok(writer) => {
                        on_output(
                            format!(
                                "\x1b[33m[警告] 串口按默认参数打开（{primary_err}）\x1b[0m\r\n"
                            )
                            .as_bytes(),
                        );
                        Ok(SerialConn {
                            writer: Some(SerialWriter::Std(writer)),
                            close_notify,
                            win_close_flag,
                        })
                    }
                    Err(_) => Err(AppError::Connection(format!("{path}: {primary_err}"))),
                }
            }
        }
    }
}

#[async_trait]
impl TerminalStream for SerialConn {
    async fn write(&mut self, data: &[u8]) -> AppResult<()> {
        match self.writer.as_mut().ok_or_else(|| AppError::Connection("closed".into()))? {
            SerialWriter::Tokio(w) => w.write_all(data).await?,
            SerialWriter::Std(file) => {
                let f = file.clone();
                let owned = data.to_vec();
                tokio::task::spawn_blocking(move || -> Result<(), AppError> {
                    use std::io::Write;
                    let mut f = f.lock().map_err(|e| AppError::Connection(e.to_string()))?;
                    f.write_all(&owned).map_err(AppError::Io)?;
                    f.flush().map_err(AppError::Io)?;
                    Ok(())
                })
                .await
                .map_err(|e| AppError::Connection(e.to_string()))??;
            }
        }
        Ok(())
    }

    async fn close(&mut self) -> AppResult<()> {
        match self.writer.take() {
            Some(SerialWriter::Tokio(mut w)) => {
                let _ = w.shutdown().await;
            }
            Some(SerialWriter::Std(_)) | None => {}
        }
        self.win_close_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.close_notify.notify_one();
        Ok(())
    }
}

// Windows 降级路径：绕过 serialport-rs，手动 Win32 打开（容忍 SetCommState 失败）。
#[cfg(windows)]
mod win_fallback {
    use std::fs::File;
    use std::os::windows::io::{FromRawHandle, IntoRawHandle};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use crate::error::{AppError, AppResult};
    use crate::protocol::{ClosedSignal, OutputSink};

    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    type Handle = *mut core::ffi::c_void;

    #[repr(C)]
    struct Dcb {
        dcb_length: u32,
        baud_rate: u32,
        flags: u32,
        w_reserved: u16,
        xon_lim: u16,
        xoff_lim: u16,
        byte_size: u8,
        parity: u8,
        stop_bits: u8,
        xon_char: u8,
        xoff_char: u8,
        error_char: u8,
        eof_char: u8,
        evt_char: u8,
        w_reserved1: u16,
    }

    #[repr(C)]
    struct CommTimeouts {
        read_interval: u32,
        read_total_multiplier: u32,
        read_total_constant: u32,
        write_total_multiplier: u32,
        write_total_constant: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            lpfilename: *const u16,
            dwdesiredaccess: u32,
            dwsharemode: u32,
            lpsecurityattributes: *mut core::ffi::c_void,
            dwcreationdisposition: u32,
            dwflagsandattributes: u32,
            htemplatefile: *mut core::ffi::c_void,
        ) -> Handle;
        fn GetCommState(handle: Handle, dcb: *mut Dcb) -> i32;
        fn SetCommState(handle: Handle, dcb: *mut Dcb) -> i32;
        fn SetCommTimeouts(handle: Handle, t: *mut CommTimeouts) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    const GENERIC_READ_WRITE: u32 = 0xC000_0000;
    const OPEN_EXISTING: u32 = 3;

    pub fn open_blocking(
        path: &str,
        baud: u32,
        close_flag: Arc<AtomicBool>,
        on_output: OutputSink,
        on_closed: ClosedSignal,
    ) -> AppResult<Arc<std::sync::Mutex<File>>> {
        let device_name = format!(r"\\.\{path}");
        let mut device: Vec<u16> = OsStr::new(&device_name)
            .encode_wide()
            .collect();
        device.push(0);

        let handle = unsafe {
            CreateFileW(
                device.as_ptr(),
                GENERIC_READ_WRITE,
                0,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle as isize == -1 {
            return Err(AppError::Connection(format!("{path}: CreateFile failed")));
        }
        let guard = Guard(handle);

        unsafe {
            let mut dcb: Dcb = std::mem::zeroed();
            dcb.dcb_length = std::mem::size_of::<Dcb>() as u32;
            if GetCommState(guard.0, &mut dcb) == 0 {
                return Err(AppError::Connection(format!("{path}: GetCommState failed")));
            }
            let current_baud = dcb.baud_rate;

            // 尝试应用请求的波特率；驱动拒绝（如 CH341SER 坏驱动）则保留默认并在终端提示
            if current_baud != baud && SetCommState(guard.0, &mut dcb) == 0 {
                let warn = format!(
                    "\r\n\x1b[33m[警告] 串口参数应用失败，使用端口当前参数 {current_baud} 8N1\x1b[0m\r\n"
                );
                on_output(warn.as_bytes());
            }

            let mut timeouts = CommTimeouts {
                read_interval: u32::MAX,
                read_total_multiplier: u32::MAX,
                read_total_constant: 500,
                write_total_multiplier: 0,
                write_total_constant: 5000,
            };
            if SetCommTimeouts(guard.0, &mut timeouts) == 0 {
                return Err(AppError::Connection(format!(
                    "{path}: SetCommTimeouts failed"
                )));
            }

            // reader 持有原始句柄（唯一属主），writer 用 try_clone 出的独立句柄
            let reader = File::from_raw_handle(guard.0 as std::os::windows::io::RawHandle);
            std::mem::forget(guard);
            let writer = Arc::new(std::sync::Mutex::new(File::from_raw_handle(
                reader
                    .try_clone()
                    .map_err(|e| AppError::Connection(e.to_string()))?
                    .into_raw_handle(),
            )));

            tokio::task::spawn_blocking(move || {
                read_loop(reader, close_flag, on_output, on_closed);
            });

            Ok(writer)
        }
    }

    struct Guard(Handle);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    fn read_loop(
        mut file: File,
        close_flag: Arc<AtomicBool>,
        on_output: OutputSink,
        on_closed: ClosedSignal,
    ) {
        use std::io::Read;
        let mut buf = [0u8; 4096];
        loop {
            if close_flag.load(Ordering::Relaxed) {
                on_closed();
                return;
            }
            match file.read(&mut buf) {
                Ok(0) => {
                    // 超时配置下的空读是正常轮询节拍，仅在主动关闭时退出
                    if close_flag.load(Ordering::Relaxed) {
                        on_closed();
                        return;
                    }
                }
                Ok(n) => on_output(&buf[..n]),
                Err(_) => {
                    on_closed();
                    return;
                }
            }
        }
    }
}
