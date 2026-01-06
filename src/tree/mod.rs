use std::ffi::OsString;
use std::io;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::CompressionKind;
use crate::stream::Stream;

#[cfg(feature = "reqwest")]
mod download;

#[derive(Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct Tree {
    pub permissions: u32,
    pub streams: Vec<Stream>,
    pub subtrees: Vec<(PathBuf, Tree)>,
    pub symlinks: Vec<Symlink>,
}

#[derive(Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct Symlink {
    pub file_name: OsString,
    pub target: PathBuf,
}

impl Tree {
    #[must_use]
    pub fn all_streams(&self) -> Vec<Stream> {
        let mut streams = self.streams.clone();

        for tree in &self.subtrees {
            let added_streams = tree.1.all_streams();
            streams.extend_from_slice(&added_streams);
        }

        streams
    }

    /// # Warning
    ///
    /// - Make sure that the tree is likely to be on the same partition as the store, as this internally uses
    ///   hardlinks and falls back onto copying, which will be expensive.
    ///
    /// # Errors
    ///
    /// - Out of storage/Permissions Errors
    pub fn deploy(&self, stream_dir: &Path, deploy_path: &Path) -> crate::Result<()> {
        for subtree in &self.subtrees {
            let next_deploy_path = &deploy_path.join(&subtree.0);
            std::fs::create_dir_all(next_deploy_path)?;
            subtree.1.deploy(stream_dir, next_deploy_path)?;
        }

        for stream in &self.streams {
            let original_path = stream_dir.join(stream.raw_filename());
            let target_path = deploy_path.join(&stream.file_name);

            if std::fs::hard_link(&original_path, &target_path).is_err() {
                std::fs::copy(&original_path, &target_path)?;
            }
        }

        for link in &self.symlinks {
            symlink(&link.target, &link.file_name)?;
        }

        Ok(())
    }

    /// Create a `Tree` and the underlying `Stream`s inside the `Repository`.
    ///
    /// # Errors
    ///
    /// - Out of storage/Permissions Errors
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
#[cfg(feature = "reqwest")]
mod tests {
    use httpmock::{Mock, prelude::*};
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
        let a_contents = b"contents";
        fs::write(original_path.join("file"), a_contents).await?;

        std::fs::create_dir_all(original_path.join("a/b"))?;

        let b_contents = b"other_contents";
        fs::write(original_path.join("a/b/c"), b_contents).await?;

        // Create a tree and host it on a mock server
        let tree = Tree::create(remote_stream_path, original_path, compression).await?;

        let server = MockServer::start();
        let mocks: Vec<Mock> = tree
            .all_streams()
            .into_iter()
            .map(|stream| {
                server.mock(|when, then| {
                    when.method(GET)
                        .path(format!("/{}.zstd", stream.raw_filename()));
                    then.status(200).body_from_file(
                        remote_stream_path
                            .join(format!("{}.zstd", stream.raw_filename()))
                            .to_str()
                            .expect("non unicode path to testdir"),
                    );
                })
            })
            .collect();

        // Download the streams from the mock server, and ensure it was accessed
        tree.download(&server.base_url(), local_stream_path, compression)
            .await?;

        for mock in mocks {
            mock.assert();
        }

        // Deploy the mock server
        tree.deploy(local_stream_path, deploy_path)?;

        // Ensure everything is correct
        assert_eq!(
            fs::oneshot::read_to_end(deploy_path.join("file")).await?,
            a_contents
        );
        assert_eq!(
            fs::oneshot::read_to_end(deploy_path.join("a/b/c")).await?,
            b_contents
        );

        Ok(())
    }

    #[test]
    fn test_all_streams() {
        let mut streams = [
            Stream {
                hash: "a".to_string(),
                file_name: "".into(),
                mode: Some(0),
                uncompressed_size: 0,
                compressed_size: 1,
            },
            Stream {
                hash: "b".to_string(),
                file_name: "".into(),
                mode: Some(0),
                uncompressed_size: 0,
                compressed_size: 2,
            },
            Stream {
                hash: "c".to_string(),
                file_name: "".into(),
                mode: Some(0),
                uncompressed_size: 0,
                compressed_size: 3,
            },
            Stream {
                hash: "d".to_string(),
                file_name: "".into(),
                mode: Some(0),
                uncompressed_size: 0,
                compressed_size: 4,
            },
        ];

        let tree = Tree {
            permissions: 0,
            streams: vec![streams[3].clone()],
            subtrees: vec![
                (
                    "a".into(),
                    Tree {
                        permissions: 0,
                        streams: vec![streams[0].clone(), streams[1].clone()],
                        subtrees: vec![],
                        symlinks: vec![],
                    },
                ),
                (
                    "b".into(),
                    Tree {
                        permissions: 0,
                        streams: vec![],
                        subtrees: vec![(
                            "c".into(),
                            Tree {
                                permissions: 0,
                                streams: vec![streams[2].clone()],
                                subtrees: vec![],
                                symlinks: vec![],
                            },
                        )],
                        symlinks: vec![],
                    },
                ),
            ],
            symlinks: vec![],
        };

        assert_eq!(tree.all_streams().len(), 4);

        let mut detected_streams = tree.all_streams();
        detected_streams.sort();
        streams.sort();

        assert_eq!(detected_streams, streams);
    }
}
