use crate::async_types::{AsyncBufRead, AsyncRead, AsyncWrite};
use crate::async_types::{Lz4Decoder, Lz4Encoder, XzDecoder, XzEncoder, ZstdDecoder, ZstdEncoder};
use std::pin::Pin;

#[derive(Copy, Clone, Debug)]
pub enum CompressionKind {
    Zstd,
    Xz,
    Lz4,
    None,
}

impl CompressionKind {
    #[must_use]
    pub fn try_get_extension(&self) -> Option<&'static str> {
        match self {
            CompressionKind::Zstd => Some("zstd"),
            CompressionKind::Lz4 => Some("lz4"),
            CompressionKind::Xz => Some("xz"),
            CompressionKind::None => None,
        }
    }

    /// WARNING: This should only be used internally, and may be removed in a future release.
    #[must_use]
    pub fn get_extension_with_dot(&self) -> String {
        match self.try_get_extension() {
            Some(e) => format!(".{e}"),
            None => String::new(),
        }
    }

    pub fn compress<'a, W: AsyncWrite + Send + 'a>(
        &self,
        sink: W,
    ) -> Pin<Box<dyn AsyncWrite + Send + 'a>> {
        match self {
            CompressionKind::Zstd => Box::pin(ZstdEncoder::new(sink)),
            CompressionKind::Xz => Box::pin(XzEncoder::new(sink)),
            CompressionKind::Lz4 => Box::pin(Lz4Encoder::new(sink)),
            CompressionKind::None => Box::pin(sink),
        }
    }

    pub fn decompress<'a, W: AsyncBufRead + Send + 'a>(
        &self,
        source: W,
    ) -> Pin<Box<dyn AsyncRead + Send + 'a>> {
        match self {
            CompressionKind::Zstd => Box::pin(ZstdDecoder::new(source)),
            CompressionKind::Xz => Box::pin(XzDecoder::new(source)),
            CompressionKind::Lz4 => Box::pin(Lz4Decoder::new(source)),
            CompressionKind::None => Box::pin(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_types::{AsyncReadExt, AsyncWriteExt, BufReader};

    #[tokio::test]
    async fn test_compression() -> Result<(), std::io::Error> {
        // Test all compression methods
        for kind in [
            CompressionKind::Zstd,
            CompressionKind::Xz,
            CompressionKind::Lz4,
            CompressionKind::None,
        ] {
            // Test random data
            for input in [
                &b"This is some test data."[..],
                &[0u8; 256][..],
                // Include LARGE data to test between "barriers"
                &[0u8; 8096],
            ] {
                // Compress
                let mut compressed_buf = Vec::new();
                let mut compressor = kind.compress(&mut compressed_buf);
                compressor.write_all(input).await?;
                #[cfg(feature = "tokio")]
                compressor.shutdown().await?;
                #[cfg(not(feature = "tokio"))]
                compressor.close().await?;
                drop(compressor);

                // Decompress
                let mut decompressor = kind.decompress(BufReader::new(&compressed_buf[..]));

                let mut decompressed_buf = Vec::new();
                decompressor.read_to_end(&mut decompressed_buf).await?;

                assert_eq!(decompressed_buf, input);
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_compression_decompress_invalid_data() {
        // Test all compression methods
        for kind in [
            CompressionKind::Zstd,
            CompressionKind::Xz,
            CompressionKind::Lz4,
        ] {
            // Test random data
            for input in [
                &b"This is some test data."[..],
                &b"I like cheese."[..],
                &b"This is more testing data."[..],
            ] {
                let mut decompressor = kind.decompress(BufReader::new(input));
                let mut decompressed_buf = Vec::new();
                let res = decompressor.read_to_end(&mut decompressed_buf).await;

                assert!(
                    res.is_err(),
                    "Compression Method: {kind:?}, Input: {input:?}"
                );
            }
        }
    }

    #[test]
    fn test_compression_filenames_with_dot() {
        assert_eq!(CompressionKind::Zstd.get_extension_with_dot(), ".zstd");
        assert_eq!(CompressionKind::Lz4.get_extension_with_dot(), ".lz4");
        assert_eq!(CompressionKind::Xz.get_extension_with_dot(), ".xz");
        assert_eq!(CompressionKind::None.get_extension_with_dot(), "");
    }

    #[test]
    fn test_compression_filenames() {
        assert_eq!(CompressionKind::Zstd.try_get_extension(), Some("zstd"));
        assert_eq!(CompressionKind::Lz4.try_get_extension(), Some("lz4"));
        assert_eq!(CompressionKind::Xz.try_get_extension(), Some("xz"));
        assert_eq!(CompressionKind::None.try_get_extension(), None);
    }
}
