//! Simple yet capable File/Tree syncing library.

mod async_types;
mod compression;
mod error;
mod fs;
pub mod stream;

pub use compression::CompressionKind;
pub use error::{Error, Result};
