// Async*, *Ext
#[cfg(not(feature = "tokio"))]
pub use futures_util::{
    AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, StreamExt, io::BufReader,
};
#[cfg(feature = "tokio")]
pub use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
#[cfg(feature = "tokio")]
pub use tokio_stream::StreamExt;

// Global
pub use futures_core::Stream;
pub use futures_util::stream::unfold;

// Async Compression
#[cfg(not(feature = "tokio"))]
pub use async_compression::futures::{
    bufread::{Lz4Decoder, XzDecoder, ZstdDecoder},
    write::{Lz4Encoder, XzEncoder, ZstdEncoder},
};
#[cfg(feature = "tokio")]
pub use async_compression::tokio::{
    bufread::{Lz4Decoder, XzDecoder, ZstdDecoder},
    write::{Lz4Encoder, XzEncoder, ZstdEncoder},
};

pub use futures_util::TryStreamExt;
