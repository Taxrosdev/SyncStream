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
        deploy_tree(&subtree.1, store_path, &deploy_path.join(&subtree.0))?;
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
