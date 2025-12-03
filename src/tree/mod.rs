use std::path::Path;

use crate::streams::{create_stream, download_stream};
use crate::types::{Compression, Error, Tree};

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

/// NOT ATOMIC
pub fn deploy_tree(tree: &Tree, store_path: &Path, deploy_path: &Path) -> Result<(), Error> {
    for subtree in &tree.subtrees {
        deploy_tree(&subtree.1, store_path, &subtree.0)?;
    }

    todo!();

    Ok(())
}

pub fn create_tree(
    repo_path: &Path,
    original_path: &Path,
    compression: Option<Compression>,
) -> Result<Tree, Error> {
    todo!()
    //for entry in Walkoriginal_path {}
}
