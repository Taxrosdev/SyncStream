use std::fs;
use std::io::Write;
use std::path::Path;

use crate::types::{CHUNK_SIZE, Chunk, Compression, Error};

pub async fn download_chunk(
    client: &reqwest::Client,
    chunk: &Chunk,
    repo_url: &str,
    compression: &Option<Compression>,
) -> Result<Vec<u8>, Error> {
    let mut compression_ext = String::new();
    if let Some(compression) = compression {
        compression_ext = format!(".{}", get_compression_extension(compression));
    }

    let res = client
        .get(format!("{repo_url}/chunks/{}{compression_ext}", chunk.hash))
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
    if hash != chunk.hash {
        return Err(Error::HashError(chunk.hash.clone(), hash));
    }

    Ok(data)
}

pub fn create_chunk(
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
        disk_size: if raw_data.len() < CHUNK_SIZE {
            Some(raw_data.len() as u64)
        } else {
            None
        },
    };

    Ok(chunk)
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
        let chunk = create_chunk(&data, repo.path(), &Some(Compression::Zstd))
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
        let new_data = download_chunk(
            &client,
            &chunk,
            &server.base_url(),
            &Some(Compression::Zstd),
        )
        .await
        .expect("could not download chunk");

        mock.assert();

        assert_eq!(data.to_vec(), new_data);
    }
}
