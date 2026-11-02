//! SOCKS5 服务端（RFC 1928）：把 SSH 会话变成动态代理，等价 `ssh -D`。
//!
//! 产品约束：只监听 127.0.0.1、不做认证、只支持 CONNECT（BIND/UDP 返回不支持）。
//! 域名原样交给远端解析 —— 不能在本地解析，否则内网 DNS/hosts 不生效。

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::profile::Socks5Config;
use crate::protocol::ssh::SshHandleSlot;

const SOCKS_VERSION: u8 = 0x05;
const METHOD_NO_AUTH: u8 = 0x00;
const METHOD_NONE_ACCEPTABLE: u8 = 0xFF;
const CMD_CONNECT: u8 = 0x01;
const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;
const REP_SUCCESS: u8 = 0x00;
const REP_FAILURE: u8 = 0x01;
const REP_HOST_UNREACHABLE: u8 = 0x04;
const REP_COMMAND_NOT_SUPPORTED: u8 = 0x07;
const REP_ADDRESS_NOT_SUPPORTED: u8 = 0x08;

/// 握手阶段读取超时（客户端迟迟不发握手/请求）
const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// 经 SSH 开通道的超时（远端不可达时不要无限等）
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// 一次 CONNECT 请求的目标
#[derive(Debug, PartialEq)]
pub struct SocksRequest {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, PartialEq)]
pub enum SocksError {
    /// 数据不足，需要继续读
    Incomplete,
    /// 连接已关闭
    Closed,
    /// 版本不是 SOCKS5
    Version,
    /// 客户端未提供「无认证」方式
    NoAcceptableMethod,
    /// 非 CONNECT（BIND / UDP ASSOCIATE）
    UnsupportedCommand,
    /// 未知地址类型
    UnsupportedAddress,
}

/// 解析握手包，返回消耗的字节数；成功即代表选择「无认证」。
pub fn parse_greeting(data: &[u8]) -> Result<usize, SocksError> {
    if data.len() < 2 {
        return Err(SocksError::Incomplete);
    }
    if data[0] != SOCKS_VERSION {
        return Err(SocksError::Version);
    }
    let count = data[1] as usize;
    if data.len() < 2 + count {
        return Err(SocksError::Incomplete);
    }
    if !data[2..2 + count].contains(&METHOD_NO_AUTH) {
        return Err(SocksError::NoAcceptableMethod);
    }
    Ok(2 + count)
}

/// 解析 CONNECT 请求，返回目标与消耗的字节数。
pub fn parse_request(data: &[u8]) -> Result<(SocksRequest, usize), SocksError> {
    if data.len() < 4 {
        return Err(SocksError::Incomplete);
    }
    if data[0] != SOCKS_VERSION {
        return Err(SocksError::Version);
    }
    if data[1] != CMD_CONNECT {
        return Err(SocksError::UnsupportedCommand);
    }
    let (host, addr_len) = match data[3] {
        ATYP_IPV4 => {
            if data.len() < 4 + 4 + 2 {
                return Err(SocksError::Incomplete);
            }
            (
                format!("{}.{}.{}.{}", data[4], data[5], data[6], data[7]),
                4,
            )
        }
        ATYP_DOMAIN => {
            if data.len() < 5 {
                return Err(SocksError::Incomplete);
            }
            let len = data[4] as usize;
            if len == 0 {
                return Err(SocksError::UnsupportedAddress);
            }
            if data.len() < 5 + len + 2 {
                return Err(SocksError::Incomplete);
            }
            (
                String::from_utf8_lossy(&data[5..5 + len]).to_string(),
                1 + len,
            )
        }
        ATYP_IPV6 => {
            if data.len() < 4 + 16 + 2 {
                return Err(SocksError::Incomplete);
            }
            let groups: Vec<String> = data[4..20]
                .chunks(2)
                .map(|c| format!("{:x}", u16::from_be_bytes([c[0], c[1]])))
                .collect();
            (groups.join(":"), 16)
        }
        _ => return Err(SocksError::UnsupportedAddress),
    };
    let port_at = 4 + addr_len;
    let port = u16::from_be_bytes([data[port_at], data[port_at + 1]]);
    Ok((SocksRequest { host, port }, port_at + 2))
}

/// 启动监听。SSH 句柄被清空（会话关闭）后停止，端口随之释放。
pub fn start(slot: SshHandleSlot, cfg: Socks5Config) {
    let Some(listen) = cfg.listen_addr() else { return };
    tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(&listen).await {
            Ok(listener) => listener,
            Err(e) => {
                crate::applog::error(&format!("SOCKS5 代理监听失败 {listen}: {e}"));
                return;
            }
        };
        crate::applog::info(&format!("SOCKS5 代理已启动 {listen}"));
        loop {
            // 注意：accept 前不抢句柄锁 —— 否则每条新连接都要等上一条的开通道 RTT，
            // 浏览器并发打开几十个连接时会严重串行（表现为页面转很久）。
            let accepted = tokio::time::timeout(
                std::time::Duration::from_millis(500),
                listener.accept(),
            )
            .await;
            match accepted {
                Ok(Ok((inbound, _peer))) => {
                    // 小包多（HTTP 请求/响应头），关掉 Nagle 降低往返延迟
                    let _ = inbound.set_nodelay(true);
                    let slot = slot.clone();
                    tokio::spawn(async move {
                        handle_conn(slot, inbound).await;
                    });
                }
                Ok(Err(e)) => {
                    crate::applog::error(&format!("SOCKS5 代理接受连接失败 {listen}: {e}"));
                    break;
                }
                // 空闲时才检查会话是否已关闭（会话关闭后 0.5s 内释放端口）
                Err(_) => {
                    if slot.lock().await.is_none() {
                        break;
                    }
                }
            }
        }
        crate::applog::info(&format!("SOCKS5 代理已停止 {listen}"));
    });
}

async fn handle_conn(slot: SshHandleSlot, mut inbound: TcpStream) {
    let mut buf: Vec<u8> = Vec::with_capacity(256);

    // 1) 握手：只接受「无认证」（带超时，避免客户端不动时长期占用）
    let greeting = match tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        read_frame(&mut inbound, &mut buf, |d| parse_greeting(d).map(|n| ((), n))),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            crate::applog::error("SOCKS5 握手超时（客户端未发送握手数据）");
            return;
        }
    };
    if greeting.is_err() {
        let _ = inbound.write_all(&[SOCKS_VERSION, METHOD_NONE_ACCEPTABLE]).await;
        return;
    }
    if inbound.write_all(&[SOCKS_VERSION, METHOD_NO_AUTH]).await.is_err() {
        return;
    }

    // 2) 请求：只接受 CONNECT
    let request = match tokio::time::timeout(HANDSHAKE_TIMEOUT, read_frame(&mut inbound, &mut buf, parse_request)).await {
        Err(_) => {
            crate::applog::error("SOCKS5 请求读取超时");
            let _ = write_reply(&mut inbound, REP_FAILURE).await;
            return;
        }
        Ok(result) => match result {
            Ok(request) => request,
            Err(SocksError::UnsupportedCommand) => {
                let _ = write_reply(&mut inbound, REP_COMMAND_NOT_SUPPORTED).await;
                return;
            }
            Err(SocksError::UnsupportedAddress) => {
                let _ = write_reply(&mut inbound, REP_ADDRESS_NOT_SUPPORTED).await;
                return;
            }
            Err(_) => {
                let _ = write_reply(&mut inbound, REP_FAILURE).await;
                return;
            }
        },
    };

    // 3) 经既有 SSH 连接开 direct-tcpip 通道（目标交给远端解析，带超时）
    let opened = tokio::time::timeout(CONNECT_TIMEOUT, async {
        let guard = slot.lock().await;
        match guard.as_ref() {
            Some(handle) => handle
                .channel_open_direct_tcpip(request.host.clone(), request.port as u32, "127.0.0.1", 0)
                .await
                .map_err(|e| e.to_string()),
            None => Err("session closed".to_string()),
        }
    })
    .await
    .unwrap_or_else(|_| Err("timeout".to_string()));
    let channel = match opened {
        Ok(channel) => channel,
        Err(e) => {
            crate::applog::error(&format!(
                "SOCKS5 打开通道失败 {}:{}: {e}",
                request.host, request.port
            ));
            let _ = write_reply(&mut inbound, REP_HOST_UNREACHABLE).await;
            return;
        }
    };
    if write_reply(&mut inbound, REP_SUCCESS).await.is_err() {
        return;
    }
    let mut outbound = channel.into_stream();
    let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
}

/// 应答：BND.ADDR 固定返回 0.0.0.0:0，客户端不依赖它
async fn write_reply(stream: &mut TcpStream, rep: u8) -> std::io::Result<()> {
    stream
        .write_all(&[SOCKS_VERSION, rep, 0x00, ATYP_IPV4, 0, 0, 0, 0, 0, 0])
        .await
}

/// 边读边解析：解析器返回 Incomplete 就继续读；成功后把已消费字节移出缓冲。
async fn read_frame<T>(
    stream: &mut TcpStream,
    buf: &mut Vec<u8>,
    parse: impl Fn(&[u8]) -> Result<(T, usize), SocksError>,
) -> Result<T, SocksError> {
    loop {
        if !buf.is_empty() {
            match parse(buf) {
                Ok((value, consumed)) => {
                    buf.drain(..consumed.min(buf.len()));
                    return Ok(value);
                }
                Err(SocksError::Incomplete) => {}
                Err(e) => return Err(e),
            }
        }
        let mut chunk = [0u8; 256];
        match stream.read(&mut chunk).await {
            Ok(0) => return Err(SocksError::Closed),
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > 1024 {
                    return Err(SocksError::UnsupportedAddress);
                }
            }
            Err(_) => return Err(SocksError::Closed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_accepts_no_auth() {
        assert_eq!(parse_greeting(b"\x05\x01\x00"), Ok(3));
        assert_eq!(parse_greeting(b"\x05\x02\x02\x00"), Ok(4));
    }

    #[test]
    fn greeting_rejects_auth_only_and_bad_version() {
        assert_eq!(parse_greeting(b"\x05\x01\x02"), Err(SocksError::NoAcceptableMethod));
        assert_eq!(parse_greeting(b"\x04\x01\x00"), Err(SocksError::Version));
    }

    #[test]
    fn greeting_waits_for_more_bytes() {
        assert_eq!(parse_greeting(b"\x05"), Err(SocksError::Incomplete));
        assert_eq!(parse_greeting(b"\x05\x02\x00"), Err(SocksError::Incomplete));
    }

    #[test]
    fn request_ipv4() {
        let data = [0x05, 0x01, 0x00, 0x01, 10, 0, 0, 5, 0x1f, 0x90];
        assert_eq!(
            parse_request(&data),
            Ok((SocksRequest { host: "10.0.0.5".into(), port: 8080 }, 10))
        );
    }

    #[test]
    fn request_domain_is_passed_through() {
        let mut data = vec![0x05, 0x01, 0x00, 0x03, 0x0b];
        data.extend_from_slice(b"example.com");
        data.extend_from_slice(&[0x00, 0x50]);
        assert_eq!(
            parse_request(&data),
            Ok((SocksRequest { host: "example.com".into(), port: 80 }, 18))
        );
    }

    #[test]
    fn request_ipv6() {
        let mut data = vec![0x05, 0x01, 0x00, 0x04];
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        data.extend_from_slice(&[0x00, 0x16]);
        assert_eq!(
            parse_request(&data),
            Ok((SocksRequest { host: "0:0:0:0:0:0:0:1".into(), port: 22 }, 22))
        );
    }

    #[test]
    fn request_rejects_udp_associate() {
        let data = [0x05, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
        assert_eq!(parse_request(&data), Err(SocksError::UnsupportedCommand));
    }

    #[test]
    fn request_rejects_unknown_atyp() {
        let data = [0x05, 0x01, 0x00, 0x09, 0, 0];
        assert_eq!(parse_request(&data), Err(SocksError::UnsupportedAddress));
    }
}
