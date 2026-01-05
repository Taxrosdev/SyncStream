use super::File;
use crate::async_types::{AsyncWrite, Stream, unfold};
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

const CHUNK_SIZE: usize = 8 * 1024;

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
    use std::io::Read;

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
