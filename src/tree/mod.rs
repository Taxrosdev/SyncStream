use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::streams::{create_stream, download_stream};
use crate::types::{Compression, Error, Symlink, Tree};

pub async fn download_tree(
    tree: &Tree,
    repo_url: &str,
    store_path: &Path,
    compression: &Option<Compression>,
) -> Result<(), Error> {
    for stream in &tree.streams {
        download_stream(stream, repo_url, store_path, compression).await?;
    }
    for tree in &tree.subtrees {
        Box::pin(download_tree(&tree.1, repo_url, store_path, compression)).await?;
    }

    Ok(())
}

/// # Warning
///
/// - This is not atomic.
///
/// - Make sure that the tree is likely to be on the same partition, as this underthehood uses
///   hardlinks and falls back onto copying, which will be much more expensive.
pub fn deploy_tree(tree: &Tree, store_path: &Path, deploy_path: &Path) -> Result<(), Error> {
    for subtree in &tree.subtrees {
        let next_deploy_path = &deploy_path.join(&subtree.0);
        fs::create_dir_all(&next_deploy_path)?;
        deploy_tree(&subtree.1, store_path, next_deploy_path)?;
    }

    for stream in &tree.streams {
        fs::hard_link(
            store_path.join(stream.get_raw_filename()),
            deploy_path.join(&stream.filename),
        )?;
    }

    Ok(())
}

pub fn create_tree(
    repo_path: &Path,
    original_path: &Path,
    compression: &Option<Compression>,
) -> Result<Tree, Error> {
    let mut base_tree = Tree {
        permissions: original_path.metadata()?.permissions().mode(),
        streams: Vec::new(),
        subtrees: Vec::new(),
        symlinks: Vec::new(),
    };

    for entry in fs::read_dir(original_path)? {
        let entry = entry?;

        let file_type = entry.file_type()?;
        let file_name = entry.file_name();

        if file_type.is_file() {
            let stream = create_stream(&entry.path(), compression, repo_path)?;
            base_tree.streams.push(stream);
        } else if file_type.is_dir() {
            let sub_tree = create_tree(repo_path, &entry.path(), compression)?;
            base_tree.subtrees.push((file_name.into(), sub_tree));
        } else if file_type.is_symlink() {
            let symlink = Symlink {
                file_name,
                target: fs::read_link(entry.path())?,
            };
            base_tree.symlinks.push(symlink);
        }
    }

    Ok(base_tree)
}

#[cfg(test)]
mod tests {
    use httpmock::prelude::*;
    use temp_dir::TempDir;

    use super::*;
    use crate::types::Compression;

    #[tokio::test]
    async fn test_e2e_tree() -> Result<(), Error> {
        let compression = &Some(Compression::Zstd);

        // Create temporary directories
        let store_dir = TempDir::new()?;
        let store_path = store_dir.path();
        let repo_dir = TempDir::new()?;
        let repo_path = repo_dir.path();

        let original_dir = TempDir::new()?;
        let original_path = original_dir.path();
        let deploy_dir = TempDir::new()?;
        let deploy_path = deploy_dir.path();

        // Create random contents
        let contents_a = "contents";
        let contents_a_hash = blake3::hash(contents_a.as_bytes()).to_hex().to_string();
        fs::write(original_path.join("file"), contents_a)?;

        fs::create_dir_all(original_path.join("a/b"))?;

        let contents_b = "other_contents";
        let contents_b_hash = blake3::hash(contents_b.as_bytes()).to_hex().to_string();
        fs::write(original_path.join("a/b/c"), contents_b)?;

        // Create a tree and host it on a mock server
        let tree = create_tree(repo_path, original_path, compression)?;

        let server = MockServer::start();
        let mock_a = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/chunks/{}.zstd", contents_a_hash));
            then.status(200).body_from_file(
                repo_path
                    .join(format!("chunks/{}.zstd", contents_a_hash))
                    .to_str()
                    .expect("non unicode path to testdir"),
            );
        });
        let mock_b = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/chunks/{}.zstd", contents_b_hash));
            then.status(200).body_from_file(
                repo_path
                    .join(format!("chunks/{}.zstd", contents_b_hash))
                    .to_str()
                    .expect("non unicode path to testdir"),
            );
        });

        // Download the chunks from the mock server, and ensure it was accessed
        download_tree(&tree, &server.base_url(), store_path, compression).await?;

        mock_a.assert();
        mock_b.assert();

        // Deploy the mock server
        deploy_tree(&tree, store_path, deploy_path)?;

        // Ensure everything is correct
        assert_eq!(fs::read_to_string(deploy_path.join("file"))?, contents_a);
        assert_eq!(fs::read_to_string(deploy_path.join("a/b/c"))?, contents_b);

        Ok(())
    }
}
