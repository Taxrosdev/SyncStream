use std::fmt;

#[derive(Debug)]
pub enum Error {
    NetworkError(reqwest::Error),
    IoError(std::io::Error),
    HashError(String, String),
    StripPrefixError(std::path::StripPrefixError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::NetworkError(ref e) => write!(f, "network error: {e}"),
            Error::IoError(ref e) => write!(f, "io error: {e}"),
            Error::HashError(ref expected, ref recieved) => {
                write!(f, "expected hash {expected}, got {recieved}")
            }
            Error::StripPrefixError(ref e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Error::NetworkError(ref e) => Some(e),
            Error::IoError(ref e) => Some(e),
            Error::HashError(_, _) => None,
            Error::StripPrefixError(ref e) => Some(e),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Error {
        Error::NetworkError(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IoError(e)
    }
}

impl From<std::path::StripPrefixError> for Error {
    fn from(e: std::path::StripPrefixError) -> Error {
        Error::StripPrefixError(e)
    }
}
