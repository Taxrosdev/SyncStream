use crate::types::Chunk;
use std::ffi::OsString;

pub mod chunk;
mod stream;

pub use stream::*;

#[derive(Clone, Debug)]
pub struct Stream {
    // The hash of the underlying file
    pub hash: String,
    // Unix mode
    pub permission: u32,
    // Within the tree, where does this fit?
    pub filename: OsString,
    // List of chunks that can be used to build the underlying file
    pub chunks: Vec<Chunk>,
}

impl Stream {
    pub fn get_raw_filename(&self) -> String {
        format!("{}{}", self.hash, self.permission.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_filename_stability() {
        let stream = Stream {
            chunks: Vec::new(),
            hash: "abc".into(),
            permission: 123,
            filename: "".into(),
        };

        let filename = stream.get_raw_filename();

        assert_eq!(filename, "abc123")
    }
}
