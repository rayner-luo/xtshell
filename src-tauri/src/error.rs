use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("unsupported protocol: {0}")]
    UnsupportedProtocol(String),
    #[error("{0}")]
    Connection(String),
    #[error("连接超时，请重试")]
    ConnectTimeout,
    #[error("无法解析主机名“{0}”，请检查地址是否正确")]
    DnsResolution(String),
    #[error("{0}")]
    Io(std::io::Error),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        match e.raw_os_error() {
            // Windows WSAETIMEDOUT / Linux ETIMEDOUT：底层 OS 文案冗长，统一为友好提示
            Some(10060) | Some(110) => Self::ConnectTimeout,
            _ => Self::Io(e),
        }
    }
}

impl AppError {
    /// 建立网络连接时的 IO 错误映射：保留主机名，供出站连接点使用。
    pub fn map_connect_io(e: std::io::Error, host: &str) -> Self {
        match e.raw_os_error() {
            // Windows WSAETIMEDOUT / Linux ETIMEDOUT：底层 OS 文案冗长，统一为友好提示
            Some(10060) | Some(110) => Self::ConnectTimeout,
            // Windows DNS 查询类错误（HOST_NOT_FOUND/TRY_AGAIN/NO_RECOVERY/NO_DATA）
            Some(11001..=11004) => Self::DnsResolution(host.to_string()),
            _ => Self::Io(e),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl From<&AppError> for ApiError {
    fn from(e: &AppError) -> Self {
        ApiError { code: "error".into(), message: e.to_string() }
    }
}

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self { ApiError::from(&e) }
}

pub type AppResult<T> = Result<T, AppError>;
