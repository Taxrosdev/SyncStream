use std::ffi::OsString;
use std::path::PathBuf;

mod errors;
pub use errors::*;

pub use crate::streams::Stream;

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

pub enum Compression {
    Zstd,
    Xz,
    Lz4,
}
