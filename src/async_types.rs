// Async*, *Ext
#[cfg(not(feature = "tokio"))]
pub use futures_util::{AsyncWrite, AsyncWriteExt, StreamExt};
#[cfg(feature = "tokio")]
pub use tokio::io::{AsyncWrite, AsyncWriteExt};
#[cfg(feature = "tokio")]
pub use tokio_stream::StreamExt;

// Global
pub use futures_core::Stream;
pub use futures_util::stream::unfold;

// Async Compression
#[cfg(not(feature = "tokio"))]
pub use async_compression::futures::write::{
    Lz4Decoder, Lz4Encoder, XzDecoder, XzEncoder, ZstdDecoder, ZstdEncoder,
};
#[cfg(feature = "tokio")]
pub use async_compression::tokio::write::{
    Lz4Decoder, Lz4Encoder, XzDecoder, XzEncoder, ZstdDecoder, ZstdEncoder,
};
