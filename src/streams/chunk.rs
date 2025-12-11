use std::fs;
use std::io::Write;
use std::path::Path;

use crate::types::{Compression, Error};

/// 16 MB
pub const CHUNK_SIZE: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Chunk {
    /// The hash of the underlying chunk file
    pub hash: String,
    /// Uncompressed size
    pub disk_size: u64,
    /// Compressed size
    pub network_size: u64,
}

impl Chunk {
    pub async fn download(
        &self,
        client: &reqwest::Client,
        repo_url: &str,
        compression: &Option<Compression>,
    ) -> Result<Vec<u8>, Error> {
        let mut compression_ext = String::new();
        if let Some(compression) = compression {
            compression_ext = format!(".{}", get_compression_extension(compression));
        }

        let res = client
            .get(format!("{repo_url}/chunks/{}{compression_ext}", self.hash))
            .send()
            .await?
            .error_for_status()?;

        let raw = res.bytes().await?;
        let raw_cursor = std::io::Cursor::new(&raw);

        let data = match compression {
            Some(Compression::Zstd) => zstd::decode_all(raw_cursor)?,
            Some(Compression::Lz4) => todo!(),
            Some(Compression::Xz) => todo!(),
            None => raw.to_vec(),
        };

        let mut hasher = blake3::Hasher::new();

        hasher.write_all(&data)?;

        let hash = hasher.finalize().to_hex().to_string();
        if hash != self.hash {
            return Err(Error::HashError(self.hash.clone(), hash));
        }

        Ok(data)
    }

    /// Create a chunk on-disk from `raw_data`, and return it's `Chunk` metadata.
    pub fn create(
        raw_data: &[u8],
        repo_path: &Path,
        compression: &Option<Compression>,
    ) -> Result<Chunk, Error> {
        let hash = blake3::hash(raw_data).to_hex().to_string();

        let chunk_dir = &repo_path.join("chunks");
        let mut chunk_path = chunk_dir.join(&hash);
        let mut chunk_path_tmp = chunk_path.clone();
        if let Some(compression) = compression {
            chunk_path.add_extension(get_compression_extension(compression));
        };
        chunk_path_tmp.add_extension("tmp");

        let data = match compression {
            Some(Compression::Zstd) => zstd::encode_all(raw_data, 3)?,
            Some(Compression::Lz4) => todo!(),
            Some(Compression::Xz) => todo!(),
            None => raw_data.to_vec(),
        };

        // Create chunk dir
        if !chunk_dir.exists() {
            fs::create_dir_all(chunk_dir)?;
        };

        fs::write(&chunk_path_tmp, &data)?;
        fs::rename(&chunk_path_tmp, chunk_path)?;

        let chunk = Chunk {
            hash,
            network_size: data.len() as u64,
            disk_size: raw_data.len() as u64,
        };

        Ok(chunk)
    }
}

fn get_compression_extension(compression: &Compression) -> &'static str {
    match compression {
        Compression::Zstd => "zstd",
        Compression::Lz4 => "lz4",
        Compression::Xz => "xz",
    }
}

#[cfg(test)]
mod tests {
    use httpmock::prelude::*;
    use temp_dir::TempDir;

    use super::*;

    #[tokio::test]
    async fn test_e2e_chunks() {
        // Generate lots of testing data
        let mut data = Vec::with_capacity(CHUNK_SIZE + 16);

        // Generating mass garbage as test data.
        for _ in 0..=data.capacity() {
            let y = getrandom::u32().expect("could not fill data with random garbage");
            data.push(y.div_ceil(8) as u8);
        }

        let repo = TempDir::new().expect("could not create temp repo");

        // Test chunking
        let chunk = Chunk::create(&data, repo.path(), &Some(Compression::Zstd))
            .expect("could not create chunk");

        // Assert that things exist on disk
        assert!(
            repo.path()
                .join("chunks")
                .join(format!("{}.zstd", chunk.hash))
                .exists()
        );

        // Test Downloading Chunks
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/chunks/{}.zstd", chunk.hash));
            then.status(200).body_from_file(
                repo.path()
                    .join(format!("chunks/{}.zstd", chunk.hash))
                    .to_str()
                    .expect("non unicode path to testdir"),
            );
        });

        let client = reqwest::Client::new();
        let new_data = chunk
            .download(&client, &server.base_url(), &Some(Compression::Zstd))
            .await
            .expect("could not download chunk");

        mock.assert();

        assert_eq!(data.to_vec(), new_data);
    }
}
