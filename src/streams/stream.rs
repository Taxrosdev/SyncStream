use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::Stream;
use super::chunk::{CHUNK_SIZE, Chunk};
use crate::hash::{HashKind, Hasher};
use crate::types::{Compression, Error};

impl Stream {
    /// Reassembles/Downloads a stream from a Repository on the internet
    pub async fn download(
        &self,
        repo_url: &str,
        store_path: &Path,
        compression: &Option<Compression>,
        hash_kind: HashKind,
    ) -> Result<(), Error> {
        let client = reqwest::Client::builder().build()?;

        let stream_path = store_path.join(self.get_raw_filename());
        let mut stream_path_tmp = stream_path.clone();
        stream_path_tmp.add_extension(".tmp");

        let mut file = File::create(&stream_path_tmp)?;
        file.lock()?;

        let mut hasher = Hasher::new(hash_kind);

        // Download and write all chunks
        for chunk in &self.chunks {
            let chunk_data = chunk
                .download(&client, repo_url, compression, hash_kind)
                .await?;
            file.write_all(&chunk_data)?;
            hasher.update(&chunk_data);
        }

        // Check the total hash
        let stream_hash = hasher.finalize();
        if stream_hash != self.hash {
            fs::remove_file(stream_path_tmp)?;
            return Err(Error::HashError(self.hash.clone(), stream_hash));
        }

        // Set permissions
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(self.permission);
        // For safety, incase the stream for whatever reason is marked as writeable
        perms.set_readonly(true);
        file.set_permissions(perms)?;

        if !stream_path.exists() {
            fs::rename(stream_path_tmp, stream_path)?;
        }

        Ok(())
    }

    /// Gets the path inside the store where the raw file is.
    pub fn get_path(&self, store_path: &Path) -> Result<PathBuf, Error> {
        let path = store_path.join(self.get_raw_filename());

        Ok(path)
    }

    /// Creates a `Stream` and its chunks.
    ///
    /// # Panics
    ///
    /// This will panic if the path points to a directory.
    ///
    /// # Arguments
    /// `path`: The actual path on disk where there is the underlying file.
    /// `compression`: Optional compression method.
    /// `repo_path`: Path to a Repository.
    pub fn create(
        path: &Path,
        repo_path: &Path,
        compression: &Option<Compression>,
        hash_kind: HashKind,
    ) -> Result<Stream, Error> {
        let file = &mut File::open(path)?;
        file.lock()?;

        // Get the mode
        let metadata = file.metadata()?;
        let mut permissions = metadata.permissions();
        permissions.set_readonly(true);
        let mode = permissions.mode();

        let mut chunks = Vec::new();
        let mut hasher = Hasher::new(hash_kind);

        loop {
            let mut chunk_buf = Vec::with_capacity(CHUNK_SIZE);
            let amount = file.take(CHUNK_SIZE as u64).read_to_end(&mut chunk_buf)?;
            if amount == 0 {
                break;
            }

            hasher.update(&chunk_buf);
            let chunk = Chunk::create(&chunk_buf, repo_path, compression, hash_kind)?;
            chunks.push(chunk);
        }

        let stream = Stream {
            filename: path.file_name().unwrap().to_os_string(),
            hash: hasher.finalize(),
            permission: mode,
            chunks,
        };

        Ok(stream)
    }
}
