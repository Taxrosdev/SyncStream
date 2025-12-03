use std::ffi::OsString;
use std::path::PathBuf;

mod errors;
pub use errors::*;

pub use crate::streams::Stream;

// 16 MB
pub const CHUNK_SIZE: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Tree {
    pub permissions: u32,
    pub streams: Vec<Stream>,
    pub subtrees: Vec<(PathBuf, Tree)>,
    pub symlinks: Vec<Symlink>,
}

#[derive(Clone, Debug)]
pub struct Symlink {
    pub file_name: OsString,
    pub target: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Chunk {
    // The hash of the underlying chunk file
    pub hash: String,
    // Only will be present on the last chunk, as an indicator of how many bytes are in that chunk
    pub disk_size: Option<u64>,
    // Will be present on every chunk
    pub network_size: u64,
}

pub enum Compression {
    Zstd,
    Xz,
    Lz4,
}
