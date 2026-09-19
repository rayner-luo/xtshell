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
