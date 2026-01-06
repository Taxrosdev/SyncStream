use std::path::Path;

use super::Tree;
use crate::CompressionKind;

impl Tree {
    /// Downloads all streams required to build the tree
    ///
    /// # Errors
    ///
    /// - Filesystem errors (Typically out of space)
    /// - Network errors (Non-2xx codes, etc)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs;

    use temp_dir::TempDir;

    #[tokio::test]
    async fn test_flat_tree() -> crate::Result<()> {
        // Build
        let remote_stream_path = TempDir::new()?;
        let remote_temp_path = TempDir::new()?;

        //std::fs::create_dir_all(remote_temp_path.child("testdir/subdir"))?;

        fs::write(&remote_temp_path.child("small_testfile"), vec![4; 4]).await?;
        fs::write(&remote_temp_path.child("big_testfile"), vec![32; 1024]).await?;

        let tree = Tree::create(
            remote_stream_path.as_ref(),
            remote_temp_path.as_ref(),
            CompressionKind::None,
        )
        .await?;

        let server = fs::serve_dir(remote_stream_path)?;

        // Download
        let local_stream_path = TempDir::new()?;

        tree.download(
            &server.base_url(),
            local_stream_path.as_ref(),
            CompressionKind::None,
        )
        .await?;

        // Assert Tests
        for stream in tree.all_streams() {
            assert!(local_stream_path.child(stream.raw_filename()).exists());
        }

        Ok(())
    }
}
