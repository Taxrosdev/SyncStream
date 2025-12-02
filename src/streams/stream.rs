use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::chunk::{create_chunk, download_chunk};
use super::get_filename;
use crate::types::{CHUNK_SIZE, Compression, Error, Stream};

// Reassembles/Downloads a stream from a Repository on the internet
pub async fn download_stream(
    stream: &Stream,
    repo_url: &str,
    store_path: &Path,
    compression: &Option<Compression>,
) -> Result<(), Error> {
    let client = reqwest::Client::builder().build()?;

    let stream_path = store_path.join(get_filename(stream));
    let mut stream_path_tmp = stream_path.clone();
    stream_path_tmp.add_extension(".tmp");

    let mut file = File::create(&stream_path_tmp)?;
    file.lock()?;

    let mut hasher = blake3::Hasher::new();

    // Download and write all chunks
    for chunk_meta in &stream.chunks {
        let chunk = download_chunk(&client, chunk_meta, repo_url, compression).await?;
        file.write_all(&chunk)?;
        hasher.write_all(&chunk)?;
    }

    // Check the total hash
    let stream_hash = hasher.finalize().to_hex().to_string();
    if stream_hash != stream.hash {
        fs::remove_file(stream_path_tmp)?;
        return Err(Error::HashError(stream.hash.clone(), stream_hash));
    }

    // Set permissions
    let mut perms = file.metadata()?.permissions();
    perms.set_mode(stream.permission);
    // For safety, incase the stream for whatever reason is marked as writeable
    perms.set_readonly(true);
    file.set_permissions(perms)?;

    if !stream_path.exists() {
        fs::rename(stream_path_tmp, stream_path)?;
    }

    Ok(())
}

/// Creates a `Stream` and its chunks.
///
/// # Arguments
/// `path`: The actual path on disk where there is the underlying file.
/// `prefix`: The prefix that should be stripped from path.
/// `compression`: Optional compression method.
/// `repo_path`: Path to a Repository.
pub fn create_stream(
    path: &Path,
    prefix: &Path,
    compression: &Option<Compression>,
    repo_path: &Path,
) -> Result<Stream, Error> {
    let file = &mut File::open(path)?;
    file.lock()?;

    // Get the mode
    let metadata = file.metadata()?;
    let mut permissions = metadata.permissions();
    permissions.set_readonly(true);
    let mode = permissions.mode();

    let mut chunks = Vec::new();
    let mut hasher = blake3::Hasher::new();

    loop {
        let mut chunk_buf = Vec::with_capacity(CHUNK_SIZE);
        let amount = file.take(CHUNK_SIZE as u64).read_to_end(&mut chunk_buf)?;
        if amount == 0 {
            break;
        }

        hasher.write_all(&chunk_buf)?;
        let chunk = create_chunk(&chunk_buf, repo_path, compression)?;
        chunks.push(chunk);
    }

    let stream = Stream {
        path: path.strip_prefix(prefix)?.to_path_buf(),
        hash: hasher.finalize().to_hex().to_string(),
        permission: mode,
        chunks,
    };

    Ok(stream)
}
