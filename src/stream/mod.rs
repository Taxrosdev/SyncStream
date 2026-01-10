use crate::async_types::{AsyncWriteExt, StreamExt};
use blake3::Hasher;
use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crate::compression::CompressionKind;
use crate::fs;

#[cfg(feature = "reqwest")]
mod download;

/// A `Stream` is an underlying representation of the underlying file, typically renamed to
/// `{hash}_{permissions}` on-disk.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Hash, Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct Stream {
    /// Blake3 Hash of underlying file
    pub hash: String,
    /// Filename of the underlying file, should not contain any '/' or directories
    pub file_name: OsString,
    /// Posix permission mode
    pub mode: Option<u32>,
    /// Uncompressed size on-disk in bytes
    pub uncompressed_size: u64,
    /// Compressed size on-disk in bytes
    /// If `CompressionKind::None`, then this is likely the same as `uncompressed_size`.
    pub compressed_size: u64,
}

impl Stream {
    /// Creates a Stream from a raw on-disk File.
    ///
    /// # Errors
    ///
    /// - Out of storage/Permissions Errors
    pub async fn create<F: AsRef<Path>, S: AsRef<Path>>(
        file: F,
        stream_dir: S,
        compression_kind: CompressionKind,
    ) -> Result<Self, std::io::Error> {
        let file_name = file
            .as_ref()
            .file_name()
            .ok_or(io::Error::from(io::ErrorKind::IsADirectory))?
            .into();

        let mut hasher = Hasher::new();

        // Hash first
        let mut stream = fs::read_chunked(&file).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.write_all(&chunk)?;
        }

        let hash = hasher.finalize().to_hex().to_string();

        // Prepare Final paths
        let mode = get_mode(&file)?;
        let uncompressed_path = stream_dir.as_ref().join(raw_filename(&hash, mode));
        let mut compressed_path = uncompressed_path.clone();
        if let Some(extension) = compression_kind.try_get_extension() {
            compressed_path.set_extension(extension);
        }

        // Check if this stream exists already
        if !uncompressed_path.exists() || !compressed_path.exists() {
            // Then Compress
            let temp_filename = blake3::hash(file.as_ref().as_os_str().as_encoded_bytes())
                .to_hex()
                .to_string();
            let mut output_temp_path = stream_dir.as_ref().join(temp_filename);
            output_temp_path.set_file_name("tmp");

            // Remove temp file if it already exists
            if output_temp_path.exists() {
                std::fs::remove_file(&output_temp_path)?;
            }

            let output_file = fs::File::create_new(&output_temp_path).await?;

            let mut writer = compression_kind.compress(output_file);

            let mut stream = fs::read_chunked(&file).await?;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                writer.write_all(&chunk).await?;
            }

            #[cfg(feature = "tokio")]
            writer.shutdown().await?;
            #[cfg(not(feature = "tokio"))]
            writer.close().await?;

            // Move/Copy to final path
            fs::rename(output_temp_path, compressed_path.clone())?;
            if std::fs::hard_link(&file, &uncompressed_path).is_err() {
                std::fs::copy(&file, &uncompressed_path)?;
            }
        }

        Ok(Self {
            hash,
            file_name,
            #[cfg(unix)]
            mode,
            uncompressed_size: std::fs::metadata(uncompressed_path)?.size(),
            compressed_size: std::fs::metadata(compressed_path)?.size(),
        })
    }

    /*
    pub async fn deploy() -> io::Result<()> {
        todo!()
    }
    */

    /// Gets the raw filesystem on-disk inside the streams directory.
    /// Typically `{file_name}_{mode}`, but this should not be relied on for future behaviour.
    #[must_use]
    pub fn raw_filename(&self) -> String {
        raw_filename(&self.hash, self.mode)
    }
}

fn raw_filename(hash: &str, mode: Option<u32>) -> String {
    if let Some(mode) = mode {
        format!("{hash}_{mode}")
    } else {
        hash.to_string()
    }
}

// Get Permissions/Mode
fn get_mode<P: AsRef<Path>>(path: P) -> io::Result<Option<u32>> {
    #[cfg(unix)]
    let mode = path.as_ref().metadata()?.mode();

    #[cfg(not(unix))]
    let mode = None;
    #[cfg(unix)]
    let mode = Some(mode);

    Ok(mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use temp_dir::TempDir;
    use temp_file::TempFile;

    #[tokio::test]
    async fn test_create_chunk_basic() -> io::Result<()> {
        let expected_hash = "477487010f611fc4cef99d0ca765636c70d84f743fb059dc5683458ad9603d54";
        let compression_kind = CompressionKind::Zstd;
        let test_data = b"This is some test data.";

        let stream_dir = TempDir::new()?;
        let test_file = TempFile::new()?.with_contents(test_data)?;

        let stream = Stream::create(test_file.path(), stream_dir.path(), compression_kind).await?;

        assert_eq!(stream.file_name, test_file.path().file_name().unwrap());
        assert_eq!(stream.hash, expected_hash);

        let mode = get_mode(test_file)?;
        let filename = raw_filename(expected_hash, mode);

        let uncompressed_file = stream_dir.path().join(filename);
        let mut compressed_file = uncompressed_file.clone();
        if let Some(extension) = compression_kind.try_get_extension() {
            compressed_file.set_extension(extension);
        }

        assert!(uncompressed_file.exists());
        assert!(compressed_file.exists());
        assert_eq!(
            fs::oneshot::read_to_end(uncompressed_file).await?,
            test_data
        );
        // TODO: Perhaps check contents of compressed?

        Ok(())
    }

    #[tokio::test]
    async fn test_create_chunk_large() -> io::Result<()> {
        let stream_dir = TempDir::new()?;

        for input in [&[][..], &[0u8; 1024][..], &[0u8; 16384][..]] {
            let compression_kind = CompressionKind::None;
            let test_file = TempFile::new()?.with_contents(input)?;

            let stream =
                Stream::create(test_file.path(), stream_dir.path(), compression_kind).await?;

            assert_eq!(stream.file_name, test_file.path().file_name().unwrap());
        }

        Ok(())
    }
}
