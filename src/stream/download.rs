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
