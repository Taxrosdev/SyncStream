use blake3::Hasher;
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use super::Stream;
use crate::CompressionKind;
use crate::async_types::{AsyncReadExt, AsyncWriteExt, BufReader, TryStreamExt};
use crate::fs;

impl Stream {
    /// Downloads this stream using reqwest
    ///
    /// # Errors
    ///
    /// - Filesystem errors (Typically out of space)
    /// - Network errors (Non-2xx codes, etc)
    #[cfg(feature = "reqwest")]
    pub async fn download<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        url: S,
        stream_dir: P,
        compression_kind: CompressionKind,
    ) -> crate::Result<PathBuf> {
        let res = reqwest::get(format!(
            "{}/streams/{}{}",
            url.as_ref(),
            self.raw_filename(),
            compression_kind.get_extension_with_dot()
        ))
        .await?;
        let res = res.error_for_status()?;

        let file_path = stream_dir.as_ref().join(self.raw_filename());
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
            .map_err(io::Error::other)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use httpmock::prelude::*;
    use temp_dir::TempDir;
    use temp_file::TempFile;

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
                .path(format!("/streams/{}.zstd", stream.raw_filename()));
            then.status(200).body_from_file(
                remote_stream_dir
                    .path()
                    .join(format!("{}.zstd", &stream.raw_filename()))
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

        let local_stream_file = local_stream_dir.path().join(stream.raw_filename());

        assert!(&local_stream_file.exists());
        assert_eq!(
            fs::oneshot::read_to_end(local_stream_file).await?,
            test_data
        );

        stream_mock.assert();

        Ok(())
    }

    #[tokio::test]
    async fn test_download_invalid_hash() -> crate::Result<()> {
        let remote_stream_dir = TempDir::new()?;
        let local_stream_dir = TempDir::new()?;
        let test_data = b"This is some test data.";
        let test_file = TempFile::new()?.with_contents(test_data)?;

        let stream = Stream::create(
            test_file.path(),
            remote_stream_dir.path(),
            CompressionKind::None,
        )
        .await?;

        fs::write(&remote_stream_dir.child(&stream.hash), "a").await?;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path(format!("/streams/{}", &stream.hash));
            then.status(200).body_from_file(
                remote_stream_dir
                    .path()
                    .join(&stream.hash)
                    .to_str()
                    .unwrap(),
            );
        });

        let res = stream
            .download(
                &server.base_url(),
                local_stream_dir.path(),
                CompressionKind::Zstd,
            )
            .await;

        assert!(res.is_err());

        Ok(())
    }
}
