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
