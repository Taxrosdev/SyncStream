use std::path::PathBuf;

mod errors;
pub use errors::*;

// 16 MB
pub const CHUNK_SIZE: usize = 16 * 1024 * 1024;

pub struct Tree {
    pub streams: Vec<Stream>,
}

#[derive(Clone, Debug)]
pub struct Stream {
    // The hash of the underlying file
    pub hash: String,
    // Unix mode
    pub permission: u32,
    // Within the tree, where does this fit?
    pub path: PathBuf,
    // List of chunks that can be used to build the underlying file
    pub chunks: Vec<Chunk>,
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
