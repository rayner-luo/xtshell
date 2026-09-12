use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("unsupported protocol: {0}")]
    UnsupportedProtocol(String),
    #[error("connection error: {0}")]
    Connection(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
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
