// Exception due to general structure needing to be the same
#![allow(clippy::unused_async)]

use crate::async_types::{AsyncWrite, AsyncWriteExt, Stream, unfold};
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

#[cfg(not(feature = "tokio"))]
use futures_util::io::AllowStdIo;

#[cfg(not(feature = "tokio"))]
use std::io::Read;

const CHUNK_SIZE: usize = 8 * 1024;

pub struct File {
    inner: Pin<Box<dyn AsyncWrite + Send + Unpin>>,
}

impl File {
    #[cfg(feature = "tokio")]
    pub async fn create_new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let inner = tokio::fs::File::create_new(path).await?;

        Ok(Self {
            inner: Box::pin(inner),
        })
    }

    #[cfg(not(feature = "tokio"))]
    pub async fn create_new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let inner = std::fs::File::create_new(path)?;
        let inner = AllowStdIo::new(inner);

        Ok(Self {
            inner: Box::pin(inner),
        })
    }
}

impl AsyncWrite for File {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    #[cfg(feature = "tokio")]
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    #[cfg(not(feature = "tokio"))]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_close(cx)
    }
}

/// Not recommended outside of tests, as loads entire file into memory.
pub async fn read_to_end<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, std::io::Error> {
    #[cfg(feature = "tokio")]
    let data = tokio::fs::read(path).await?;
    #[cfg(not(feature = "tokio"))]
    let data = std::fs::read(path)?;

    Ok(data)
}

pub async fn remove_file<P: AsRef<Path>>(path: P) -> Result<(), std::io::Error> {
    #[cfg(feature = "tokio")]
    tokio::fs::remove_file(path).await?;
    #[cfg(not(feature = "tokio"))]
    std::fs::remove_file(path)?;
    Ok(())
}

#[cfg(feature = "tokio")]
pub async fn read_chunked<P: AsRef<Path>>(
    path: P,
) -> io::Result<Pin<Box<impl Stream<Item = io::Result<Vec<u8>>>>>> {
    use tokio::io::AsyncReadExt;

    let file = tokio::fs::File::open(path).await?;

    Ok(Box::pin(unfold(file, |mut file| async move {
        let mut buf = vec![0; CHUNK_SIZE];

        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(buf), file))
            }
            Err(e) => Some((Err(e), file)),
        }
    })))
}

#[cfg(not(feature = "tokio"))]
pub async fn read_chunked<P: AsRef<Path>>(
    path: P,
) -> io::Result<Pin<Box<impl Stream<Item = io::Result<Vec<u8>>>>>> {
    let file = std::fs::File::open(path)?;

    Ok(Box::pin(unfold(file, |mut file| async move {
        let mut buf = vec![0; CHUNK_SIZE];

        match file.read(&mut buf) {
            Ok(0) => None, // EOF → end stream
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(buf), file))
            }
            Err(e) => Some((Err(e), file)),
        }
    })))
}

pub async fn write<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    contents: C,
) -> Result<(), std::io::Error> {
    #[cfg(feature = "tokio")]
    tokio::fs::write(path, contents).await?;
    #[cfg(not(feature = "tokio"))]
    std::fs::write(path, contents)?;

    Ok(())
}

/// Atomic Rename (on supported platforms)
#[cfg(unix)]
pub fn rename<P: AsRef<Path>>(original_path: P, new_path: P) -> io::Result<()> {
    use nix::fcntl::{AT_FDCWD, RenameFlags, renameat2};

    let is_dir = original_path.as_ref().metadata()?.is_dir();

    if !new_path.as_ref().exists() {
        if is_dir {
            std::fs::create_dir_all(&new_path)?;
        } else {
            std::fs::write(&new_path, b"")?;
        }
    }

    renameat2(
        AT_FDCWD,
        original_path.as_ref(),
        AT_FDCWD,
        new_path.as_ref(),
        RenameFlags::RENAME_EXCHANGE,
    )?;

    if original_path.as_ref().exists() {
        if is_dir {
            std::fs::remove_dir_all(original_path)?;
        } else {
            std::fs::remove_file(original_path)?;
        }
    }

    Ok(())
}

/// Atomic Rename (on supported platforms)
#[cfg(not(unix))]
pub fn rename<P: AsRef<Path>>(original_path: P, new_path: P) -> io::Result<()> {
    if new_path.as_ref().exists() {
        if new_path.as_ref().is_file() {
            std::fs::remove_file(&new_path)?;
        } else {
            std::fs::remove_dir_all(&new_path)?;
        }
    }

    std::fs::rename(original_path, new_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use temp_dir::TempDir;
    use temp_file::TempFile;

    #[tokio::test]
    async fn test_fs_rw_to_end() -> io::Result<()> {
        let data = b"This is some test data.";
        let file = TempFile::new()?;

        write(&file, data).await?;
        let new_data = read_to_end(&file).await?;

        assert_eq!(data, &new_data[..]);

        Ok(())
    }

    #[tokio::test]
    async fn test_fs_rw_chunked() -> io::Result<()> {
        let data = b"This is some test data.";
        let file = TempFile::new()?;

        write(&file, data).await?;

        let mut buf = Vec::new();
        let mut stream = read_chunked(&file).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buf.extend_from_slice(&chunk);
        }

        assert_eq!(data, &buf[..]);

        Ok(())
    }

    #[tokio::test]
    async fn test_rename_basic() -> io::Result<()> {
        let dir = TempDir::new()?;
        let new_file = dir.path().join("target.new");
        write(&new_file, "new data").await?;
        let target_file = dir.path().join("target");

        rename(&new_file, &target_file)?;

        assert_eq!(read_to_end(target_file).await?, b"new data");
        assert!(!new_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_rename_atomic_replace() -> io::Result<()> {
        let dir = TempDir::new()?;
        let new_file = dir.path().join("target.new");
        write(&new_file, "new data").await?;
        let target_file = dir.path().join("target");
        write(&target_file, "old data").await?;

        rename(&new_file, &target_file)?;

        assert_eq!(read_to_end(target_file).await?, b"new data");
        assert!(!new_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_basic_file() -> io::Result<()> {
        let test_data = b"This is some test data.";
        let dir = TempDir::new()?;
        let file_name = "test_file";
        let file_path = dir.path().join(file_name);

        // Effectively the entire test
        let mut file = File::create_new(&file_path).await?;
        file.write(test_data).await?;
        drop(file);

        assert!(file_path.exists());
        assert_eq!(read_to_end(file_path).await?, test_data);

        Ok(())
    }
}
