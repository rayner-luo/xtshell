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
        let raw_data_bits = data_bits;
        let raw_stop_bits = stop_bits;
        let raw_parity = parity;

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
                match win_fallback::open_blocking(
                    path,
                    baud,
                    raw_data_bits,
                    raw_stop_bits,
                    raw_parity,
                    win_close_flag.clone(),
                    on_output.clone(),
                    on_closed,
                ) {
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
        data_bits: Option<u8>,
        stop_bits: Option<u8>,
        parity: Option<&str>,
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

            // 应用请求参数（波特率/数据位/停止位/校验 + 清零流控）；
            // 驱动拒绝（如 CH341SER 坏驱动）则保留当前参数，并在终端提示实际生效值。
            let requested = params_desc(baud, data_bits, stop_bits, parity);
            dcb = apply_params(dcb, baud, data_bits, stop_bits, parity);
            if SetCommState(guard.0, &mut dcb) == 0 {
                let actual = {
                    let mut d: Dcb = std::mem::zeroed();
                    d.dcb_length = std::mem::size_of::<Dcb>() as u32;
                    if GetCommState(guard.0, &mut d) == 0 {
                        dcb_desc(&d)
                    } else {
                        "未知".to_string()
                    }
                };
                let warn = format!(
                    "\r\n\x1b[33m[警告] 串口参数应用失败：请求 {requested}，当前实际 {actual}\x1b[0m\r\n"
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

    // DCB flags 位（对齐 Win32 DCB 定义）
    const FLAG_BINARY: u32 = 1; // fBinary (bit 0)
    const FLAG_PARITY: u32 = 1 << 1; // fParity (bit 1)

    const PARITY_NONE: u8 = 0;
    const PARITY_ODD: u8 = 1;
    const PARITY_EVEN: u8 = 2;
    const STOP_BITS_ONE: u8 = 0;
    const STOP_BITS_TWO: u8 = 2;

    /// 把请求的串口参数写入 DCB：波特率/数据位/停止位/校验，并清零流控位
    /// （与主路径 serialport-rs 的 `FlowControl::None` 语义对齐：CTS/DSR/软件流控全关，
    /// RTS/DTR 均 Disable）。纯函数不接触句柄，便于单测。
    fn apply_params(
        mut dcb: Dcb,
        baud: u32,
        data_bits: Option<u8>,
        stop_bits: Option<u8>,
        parity: Option<&str>,
    ) -> Dcb {
        dcb.baud_rate = baud;
        dcb.byte_size = match data_bits {
            Some(5) => 5,
            Some(6) => 6,
            Some(7) => 7,
            _ => 8,
        };
        dcb.stop_bits = match stop_bits {
            Some(2) => STOP_BITS_TWO,
            _ => STOP_BITS_ONE,
        };
        dcb.parity = match parity {
            Some("Odd") => PARITY_ODD,
            Some("Even") => PARITY_EVEN,
            _ => PARITY_NONE,
        };
        // fBinary 必须置位；fParity 依校验位；流控位清零（RTS/DTR Disable）
        dcb.flags = FLAG_BINARY
            | if dcb.parity == PARITY_NONE { 0 } else { FLAG_PARITY };
        dcb
    }

    fn parity_char(p: u8) -> char {
        match p {
            PARITY_ODD => 'O',
            PARITY_EVEN => 'E',
            3 => 'M',
            4 => 'S',
            _ => 'N',
        }
    }

    fn dcb_desc(dcb: &Dcb) -> String {
        let stop = if dcb.stop_bits == STOP_BITS_TWO { "2" } else { "1" };
        format!("{} {}{}{}", dcb.baud_rate, dcb.byte_size, parity_char(dcb.parity), stop)
    }

    fn params_desc(baud: u32, data_bits: Option<u8>, stop_bits: Option<u8>, parity: Option<&str>) -> String {
        let byte = match data_bits {
            Some(5) => 5,
            Some(6) => 6,
            Some(7) => 7,
            _ => 8,
        };
        let p = match parity {
            Some("Odd") => 'O',
            Some("Even") => 'E',
            _ => 'N',
        };
        let stop = if stop_bits == Some(2) { "2" } else { "1" };
        format!("{baud} {byte}{p}{stop}")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const FLAG_OUTX_CTS_FLOW: u32 = 1 << 2; // fOutxCtsFlow (bit 2)
        const FLAG_OUTX_DSR_FLOW: u32 = 1 << 3; // fOutxDsrFlow (bit 3)
        const FLAG_OUT_X: u32 = 1 << 8; // fOutX (bit 8)
        const FLAG_IN_X: u32 = 1 << 9; // fInX (bit 9)

        fn base_dcb() -> Dcb {
            let mut dcb: Dcb = unsafe { std::mem::zeroed() };
            dcb.dcb_length = std::mem::size_of::<Dcb>() as u32;
            // 模拟驱动默认：9600 8N1，且带 CTS/DSR/软件流控
            dcb.baud_rate = 9600;
            dcb.byte_size = 8;
            dcb.parity = PARITY_NONE;
            dcb.stop_bits = STOP_BITS_ONE;
            dcb.flags = FLAG_BINARY | FLAG_OUTX_CTS_FLOW | FLAG_OUTX_DSR_FLOW | FLAG_OUT_X | FLAG_IN_X;
            dcb
        }

        #[test]
        fn applies_requested_params_and_clears_flow_control() {
            let dcb = apply_params(base_dcb(), 115200, Some(8), Some(1), None);
            assert_eq!(dcb.baud_rate, 115200);
            assert_eq!(dcb.byte_size, 8);
            assert_eq!(dcb.stop_bits, STOP_BITS_ONE);
            assert_eq!(dcb.parity, PARITY_NONE);
            assert_eq!(dcb.flags & FLAG_OUTX_CTS_FLOW, 0);
            assert_eq!(dcb.flags & FLAG_OUTX_DSR_FLOW, 0);
            assert_eq!(dcb.flags & FLAG_OUT_X, 0);
            assert_eq!(dcb.flags & FLAG_IN_X, 0);
            assert_eq!(dcb.flags & FLAG_BINARY, FLAG_BINARY);
        }

        #[test]
        fn applies_even_parity_and_two_stop_bits() {
            let dcb = apply_params(base_dcb(), 9600, Some(7), Some(2), Some("Even"));
            assert_eq!(dcb.byte_size, 7);
            assert_eq!(dcb.stop_bits, STOP_BITS_TWO);
            assert_eq!(dcb.parity, PARITY_EVEN);
            assert_eq!(dcb.flags & FLAG_PARITY, FLAG_PARITY);
        }

        #[test]
        fn defaults_to_8n1() {
            let dcb = apply_params(base_dcb(), 9600, None, None, None);
            assert_eq!(dcb.byte_size, 8);
            assert_eq!(dcb.stop_bits, STOP_BITS_ONE);
            assert_eq!(dcb.parity, PARITY_NONE);
            assert_eq!(dcb.flags & FLAG_PARITY, 0);
        }

        #[test]
        fn unsupported_parity_falls_back_to_none() {
            let dcb = apply_params(base_dcb(), 9600, None, None, Some("Mark"));
            assert_eq!(dcb.parity, PARITY_NONE);
            assert_eq!(dcb.flags & FLAG_PARITY, 0);
        }
    }
}
