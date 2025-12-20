use std::ffi::OsString;
use std::io;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::CompressionKind;
use crate::stream::Stream;

#[derive(Clone, Debug, Hash)]
pub struct Tree {
    pub permissions: u32,
    pub streams: Vec<Stream>,
    pub subtrees: Vec<(PathBuf, Tree)>,
    pub symlinks: Vec<Symlink>,
}

#[derive(Clone, Debug, Hash)]
pub struct Symlink {
    pub file_name: OsString,
    pub target: PathBuf,
}

impl Tree {
    /// Downloads all streams required to build the tree
    pub async fn download(
        &self,
        repo_url: &str,
        local_stream_path: &Path,
        compression: CompressionKind,
    ) -> crate::Result<()> {
        for stream in &self.streams {
            stream
                .download(repo_url, local_stream_path, compression)
                .await?;
        }
        for tree in &self.subtrees {
            Box::pin(tree.1.download(repo_url, local_stream_path, compression)).await?;
        }

        Ok(())
    }

    /// # Warning
    ///
    /// - Make sure that the tree is likely to be on the same partition as the store, as this internally uses
    ///   hardlinks and falls back onto copying, which will be expensive.
    pub fn deploy(&self, stream_dir: &Path, deploy_path: &Path) -> crate::Result<()> {
        for subtree in &self.subtrees {
            let next_deploy_path = &deploy_path.join(&subtree.0);
            std::fs::create_dir_all(next_deploy_path)?;
            subtree.1.deploy(stream_dir, next_deploy_path)?;
        }

        for stream in &self.streams {
            let original_path = stream_dir.join(&stream.hash);
            let target_path = deploy_path.join(&stream.file_name);

            if std::fs::hard_link(&original_path, &target_path).is_err() {
                std::fs::copy(&original_path, &target_path)?;
            };
        }

        for link in &self.symlinks {
            symlink(&link.target, &link.file_name)?;
        }

        Ok(())
    }

    /// Create a `Tree` and the underlying `Stream`s inside the `Repository`.
    pub async fn create(
        remote_stream_path: &Path,
        original_path: &Path,
        compression: CompressionKind,
    ) -> io::Result<Tree> {
        let mut base_tree = Tree {
            permissions: original_path.metadata()?.permissions().mode(),
            streams: Vec::new(),
            subtrees: Vec::new(),
            symlinks: Vec::new(),
        };

        for entry in std::fs::read_dir(original_path)? {
            let entry = entry?;

            let file_type = entry.file_type()?;
            let file_name = entry.file_name();

            if file_type.is_file() {
                let stream =
                    Stream::create(&entry.path(), &remote_stream_path, compression).await?;
                base_tree.streams.push(stream);
            } else if file_type.is_dir() {
                let sub_tree =
                    Box::pin(Tree::create(remote_stream_path, &entry.path(), compression)).await?;
                base_tree.subtrees.push((file_name.into(), sub_tree));
            } else if file_type.is_symlink() {
                let symlink = Symlink {
                    file_name,
                    target: std::fs::read_link(entry.path())?,
                };
                base_tree.symlinks.push(symlink);
            }
        }

        Ok(base_tree)
    }
}

#[cfg(test)]
mod tests {
    use httpmock::prelude::*;
    use temp_dir::TempDir;

    use super::*;
    use crate::CompressionKind;
    use crate::fs;

    #[tokio::test]
    async fn test_e2e_tree() -> crate::Result<()> {
        let compression = CompressionKind::Zstd;

        // Create temporary directories
        let local_stream_dir = TempDir::new()?;
        let local_stream_path = local_stream_dir.path();
        let remote_stream_dir = TempDir::new()?;
        let remote_stream_path = remote_stream_dir.path();

        let original_dir = TempDir::new()?;
        let original_path = original_dir.path();
        let deploy_dir = TempDir::new()?;
        let deploy_path = deploy_dir.path();

        // Create random contents
        let contents_a = b"contents";
        let contents_a_hash = blake3::hash(contents_a).to_hex().to_string();
        fs::write(original_path.join("file"), contents_a).await?;

        std::fs::create_dir_all(original_path.join("a/b"))?;

        let contents_b = b"other_contents";
        let contents_b_hash = blake3::hash(contents_b).to_hex().to_string();
        fs::write(original_path.join("a/b/c"), contents_b).await?;

        // Create a tree and host it on a mock server
        let tree = Tree::create(remote_stream_path, original_path, compression).await?;

        let server = MockServer::start();
        let mock_a = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/streams/{}.zstd", contents_a_hash));
            then.status(200).body_from_file(
                remote_stream_path
                    .join(format!("{}.zstd", contents_a_hash))
                    .to_str()
                    .expect("non unicode path to testdir"),
            );
        });
        let mock_b = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/streams/{}.zstd", contents_b_hash));
            then.status(200).body_from_file(
                remote_stream_path
                    .join(format!("{}.zstd", contents_b_hash))
                    .to_str()
                    .expect("non unicode path to testdir"),
            );
        });

        // Download the streams from the mock server, and ensure it was accessed
        tree.download(&server.base_url(), local_stream_path, compression)
            .await?;

        mock_a.assert();
        mock_b.assert();

        // Deploy the mock server
        tree.deploy(local_stream_path, deploy_path)?;

        // Ensure everything is correct
        assert_eq!(fs::read_to_end(deploy_path.join("file")).await?, contents_a);
        assert_eq!(
            fs::read_to_end(deploy_path.join("a/b/c")).await?,
            contents_b
        );

        Ok(())
    }
}
