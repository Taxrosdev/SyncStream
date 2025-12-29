#![doc = include_str!("../README.md")]

mod async_types;
mod compression;
mod error;
mod fs;
pub mod stream;
pub mod tree;

pub use compression::CompressionKind;
pub use error::{Error, Result};
