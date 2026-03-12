use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
}
