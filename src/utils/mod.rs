use nix::fcntl::{AT_FDCWD, RenameFlags, renameat2};
use std::fs;
use std::path::Path;

pub fn rename_atomic(old_path: &Path, new_path: &Path) -> Result<(), std::io::Error> {
    let is_dir = old_path.metadata()?.is_dir();

    if !new_path.exists() {
        if is_dir {
            fs::create_dir_all(new_path)?;
        } else {
            fs::write(new_path, b"")?;
        }
    }

    renameat2(
        AT_FDCWD,
        old_path,
        AT_FDCWD,
        new_path,
        RenameFlags::RENAME_EXCHANGE,
    )?;

    if old_path.exists() {
        if is_dir {
            fs::remove_dir_all(old_path)?;
        } else {
            fs::remove_file(old_path)?;
        }
    }

    Ok(())
}
