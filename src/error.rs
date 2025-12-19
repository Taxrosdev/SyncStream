pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    NetworkError(reqwest::Error),
    /// Expected and Recieved
    HashError(String, String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Error::IoError(err) => write!(formatter, "io error: {err}"),
            Error::NetworkError(err) => write!(formatter, "network error: {err}"),
            Error::HashError(expected, recieved) => write!(
                formatter,
                "hash error: expected {expected}, recieved {recieved}"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::NetworkError(err)
    }
}
