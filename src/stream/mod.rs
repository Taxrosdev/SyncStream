use crate::async_types::{AsyncWriteExt, StreamExt};
use blake3::Hasher;
use std::ffi::OsString;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::compression::{CompressionKind, compressor};
use crate::fs;

pub struct Stream {
    hash: String,
    filename: OsString,
}

impl Stream {
    /// Downloads this stream using reqwest
    pub async fn download<P: AsRef<Path>, S: AsRef<str>>(
        &self,
        url: S,
        stream_dir: P,
    ) -> crate::Result<PathBuf> {
        let res = reqwest::get(format!("{}/streams/{}", url.as_ref(), self.hash)).await?;
        let mut res = res.error_for_status()?;

        let file_path = stream_dir.as_ref().join(&self.hash);
        let mut tmp_file_path = file_path.clone();
        tmp_file_path.set_extension("tmp");
        let mut file = fs::File::create_new(&tmp_file_path).await?;

        let mut hasher = Hasher::new();

        while let Some(chunk) = res.chunk().await? {
            file.write(&chunk).await?;
            hasher.write_all(&chunk)?;
        }

        let hash = hasher.finalize().to_hex().to_string();

        if hash == self.hash {
            fs::rename(&tmp_file_path, &file_path)?;
        } else {
            fs::remove_file(tmp_file_path).await?;
        }

        Ok(file_path)
    }

    pub async fn create<P: AsRef<Path>>(
        file: P,
        stream_dir: P,
        compression_kind: CompressionKind,
    ) -> Result<Self, std::io::Error> {
        let filename = file
            .as_ref()
            .file_name()
            .ok_or(io::Error::from(io::ErrorKind::IsADirectory))?
            .into();

        let mut hasher = Hasher::new();

        let mut output_temp_path = stream_dir.as_ref().join(&filename);
        output_temp_path.set_file_name("tmp");
        let output_file = fs::File::create_new(&output_temp_path).await?;
        let mut writer = compressor(compression_kind, output_file);

        // Hash and compress
        let mut stream = fs::read_chunked(&file).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.write_all(&chunk)?;
            writer.write_all(&chunk).await?;
        }

        let hash = hasher.finalize().to_hex().to_string();

        // Rename to hash
        fs::rename(output_temp_path, stream_dir.as_ref().join(&hash))?;

        Ok(Self { hash, filename })
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

        let stream_dir = TempDir::new()?;
        let test_file = TempFile::new()?.with_contents(b"This is some test data.")?;

        let stream =
            Stream::create(test_file.path(), stream_dir.path(), CompressionKind::Zstd).await?;

        assert_eq!(stream.filename, test_file.path().file_name().unwrap());
        assert_eq!(stream.hash, expected_hash);

        assert!(stream_dir.path().join(expected_hash).exists());
        // TODO: Perhaps check contents and existance of only one stream?

        Ok(())
    }

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
            when.method(GET).path(format!("/streams/{}", &stream.hash));
            then.status(200).body_from_file(
                remote_stream_dir
                    .path()
                    .join(&stream.hash)
                    .to_str()
                    .unwrap(),
            );
        });

        stream
            .download(&server.base_url(), local_stream_dir.path())
            .await?;

        let local_stream_file = local_stream_dir.path().join(stream.hash);

        assert!(&local_stream_file.exists());
        assert_eq!(fs::read_to_end(local_stream_file).await?, test_data);

        Ok(())
    }
}
