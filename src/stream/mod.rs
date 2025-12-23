use crate::async_types::{AsyncReadExt, AsyncWriteExt, BufReader, StreamExt, TryStreamExt};
use blake3::Hasher;
use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crate::compression::CompressionKind;
use crate::fs;

#[derive(Hash, Clone, Debug)]
pub struct Stream {
    pub hash: String,
    pub file_name: OsString,
    #[cfg(unix)]
    pub mode: Option<u32>,
}

impl Stream {
    /// Downloads this stream using reqwest
    pub async fn download<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        url: S,
        stream_dir: P,
        compression_kind: CompressionKind,
    ) -> crate::Result<PathBuf> {
        let res = reqwest::get(format!(
            "{}/streams/{}{}",
            url.as_ref(),
            self.hash,
            compression_kind.get_extension_with_dot()
        ))
        .await?;
        let res = res.error_for_status()?;

        let file_path = stream_dir.as_ref().join(&self.hash);
        let mut tmp_file_path = file_path.clone();
        tmp_file_path.set_extension("tmp");
        let mut file = fs::File::create_new(&tmp_file_path).await?;

        let mut hasher = Hasher::new();

        #[cfg(feature = "tokio")]
        let stream =
            tokio_util::io::StreamReader::new(res.bytes_stream().map_err(io::Error::other));
        #[cfg(not(feature = "tokio"))]
        let stream = res
            .bytes_stream()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .into_async_read();

        let mut reader = compression_kind.decompress(BufReader::new(stream));

        let mut buf = [0u8; 4096];
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                break;
            }

            let chunk = &buf[..n];
            file.write_all(chunk).await?;
            hasher.write_all(chunk)?;
        }

        let hash = hasher.finalize().to_hex().to_string();

        if hash == self.hash {
            fs::rename(&tmp_file_path, &file_path)?;
            Ok(file_path)
        } else {
            fs::remove_file(tmp_file_path).await?;
            Err(crate::Error::HashError(self.hash.clone(), hash))
        }
    }

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

        // Get Permissions/Mode
        #[cfg(unix)]
        let mode = file.as_ref().metadata()?.mode();

        let mut hasher = Hasher::new();

        let mut output_temp_path = stream_dir.as_ref().join(&file_name);
        output_temp_path.set_file_name("tmp");

        let output_file = fs::File::create_new(&output_temp_path).await?;

        let mut writer = compression_kind.compress(output_file);

        // Hash and compress
        let mut stream = fs::read_chunked(&file).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.write_all(&chunk)?;
            writer.write_all(&chunk).await?;
        }

        let hash = hasher.finalize().to_hex().to_string();
        #[cfg(feature = "tokio")]
        writer.shutdown().await?;
        #[cfg(not(feature = "tokio"))]
        writer.close().await?;

        // Final paths
        let uncompressed_path = stream_dir.as_ref().join(&hash);
        let mut compressed_path = uncompressed_path.clone();
        if let Some(extension) = compression_kind.try_get_extension() {
            compressed_path.set_extension(extension);
        }

        // Move/Copy to final path
        fs::rename(output_temp_path, compressed_path)?;
        if std::fs::hard_link(&file, &uncompressed_path).is_err() {
            std::fs::copy(&file, &uncompressed_path)?;
        };

        Ok(Self {
            hash,
            file_name,
            #[cfg(unix)]
            mode: Some(mode),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
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

        let uncompressed_file = stream_dir.path().join(expected_hash);
        let mut compressed_file = uncompressed_file.clone();
        if let Some(extension) = compression_kind.try_get_extension() {
            compressed_file.set_extension(extension);
        };

        assert!(uncompressed_file.exists());
        assert!(compressed_file.exists());
        assert_eq!(fs::read_to_end(uncompressed_file).await?, test_data);
        // TODO: Perhaps check contents of compressed?

        Ok(())
    }

    #[tokio::test]
    async fn test_download_basic() -> crate::Result<()> {
        let remote_stream_dir = TempDir::new()?;
        let local_stream_dir = TempDir::new()?;
        let test_data = b"This is some test data.";
        let test_file = TempFile::new()?.with_contents(test_data)?;

        let stream = Stream::create(
            test_file.path(),
            remote_stream_dir.path(),
            CompressionKind::Zstd,
        )
        .await?;

        let server = MockServer::start();
        let stream_mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/streams/{}.zstd", &stream.hash));
            then.status(200).body_from_file(
                remote_stream_dir
                    .path()
                    .join(format!("{}.zstd", &stream.hash))
                    .to_str()
                    .unwrap(),
            );
        });

        stream
            .download(
                &server.base_url(),
                local_stream_dir.path(),
                CompressionKind::Zstd,
            )
            .await?;

        let local_stream_file = local_stream_dir.path().join(stream.hash);

        assert!(&local_stream_file.exists());
        assert_eq!(fs::read_to_end(local_stream_file).await?, test_data);

        stream_mock.assert();

        Ok(())
    }
}
