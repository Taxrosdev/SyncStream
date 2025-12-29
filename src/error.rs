pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("network error: {0:?}")]
    NetworkError(#[from] reqwest::Error),
    /// Expected and Recieved
    #[error("hash error: expected {0}, got {1}")]
    HashError(String, String),
}
