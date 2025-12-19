use crate::async_types::AsyncWrite;
use crate::async_types::{Lz4Decoder, Lz4Encoder, XzDecoder, XzEncoder, ZstdDecoder, ZstdEncoder};
use std::pin::Pin;

#[derive(Copy, Clone)]
pub enum CompressionKind {
    Zstd,
    Xz,
    Lz4,
    None,
}

pub fn compressor<'a, W: AsyncWrite + Send + 'a>(
    kind: CompressionKind,
    sink: W,
) -> Pin<Box<dyn AsyncWrite + Send + 'a>> {
    match kind {
        CompressionKind::Zstd => Box::pin(ZstdEncoder::new(sink)),
        CompressionKind::Xz => Box::pin(XzEncoder::new(sink)),
        CompressionKind::Lz4 => Box::pin(Lz4Encoder::new(sink)),
        CompressionKind::None => Box::pin(sink),
    }
}

pub fn decompressor<'a, W: AsyncWrite + Send + 'a>(
    kind: CompressionKind,
    sink: W,
) -> Pin<Box<dyn AsyncWrite + Send + 'a>> {
    match kind {
        CompressionKind::Zstd => Box::pin(ZstdDecoder::new(sink)),
        CompressionKind::Xz => Box::pin(XzDecoder::new(sink)),
        CompressionKind::Lz4 => Box::pin(Lz4Decoder::new(sink)),
        CompressionKind::None => Box::pin(sink),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_types::AsyncWriteExt;

    #[tokio::test]
    async fn test_compression() -> Result<(), std::io::Error> {
        for kind in [
            CompressionKind::Zstd,
            CompressionKind::Xz,
            CompressionKind::Lz4,
            CompressionKind::None,
        ] {
            let test_data = b"This is some test data.";

            // Compress
            let mut compressed_buf = Vec::new();
            let mut compressor = compressor(kind, &mut compressed_buf);

            compressor.write_all(&test_data[..]).await?;
            compressor.flush().await?;
            drop(compressor);

            // Decompress
            let mut decompressed_buf = Vec::new();
            let mut decompressor = decompressor(kind, &mut decompressed_buf);

            decompressor.write_all(&compressed_buf).await?;
            decompressor.flush().await?;
            drop(decompressor);

            assert_eq!(decompressed_buf, test_data);
        }

        Ok(())
    }
}
