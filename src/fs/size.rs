use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::AppError;
use crate::targets::item::{Allocation, FileIdentity};

const BLOCK_SIZE: u64 = 512;

pub fn path_allocations(path: &Path) -> Result<Vec<Allocation>, AppError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| measurement_error(path, error))?;
    if !metadata.is_dir() {
        return Ok(vec![allocation(&metadata)]);
    }

    let mut seen = BTreeSet::new();
    let mut allocations = Vec::new();

    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry.map_err(|error| AppError::Measurement {
            path: error.path().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf()),
            reason: error.to_string(),
        })?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| measurement_error(entry.path(), error))?;
        let allocation = allocation(&metadata);
        if seen.insert(allocation.identity) {
            allocations.push(allocation);
        }
    }

    allocations.sort_by_key(|allocation| allocation.identity);
    Ok(allocations)
}

fn allocation(metadata: &fs::Metadata) -> Allocation {
    Allocation {
        identity: FileIdentity { device: metadata.dev(), inode: metadata.ino() },
        bytes: metadata.blocks().saturating_mul(BLOCK_SIZE),
    }
}

fn measurement_error(path: &Path, error: std::io::Error) -> AppError {
    AppError::Measurement { path: path.to_path_buf(), reason: error.to_string() }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;

    #[test]
    fn path_allocations_count_hard_links_once() {
        let temp = TempDir::new().expect("temp directory is created");
        let first = temp.child("first.bin");
        first.write_binary(&vec![1; 8192]).expect("first file exists");
        let second = temp.child("second.bin");
        std::fs::hard_link(first.path(), second.path()).expect("hard link exists");

        let allocations = path_allocations(temp.path()).expect("directory is measured");
        let file_identity = FileIdentity {
            device: first.metadata().expect("metadata exists").dev(),
            inode: first.metadata().expect("metadata exists").ino(),
        };

        assert_eq!(
            allocations.iter().filter(|allocation| allocation.identity == file_identity).count(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn path_allocations_do_not_follow_symbolic_links() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp directory is created");
        let outside = TempDir::new().expect("outside directory is created");
        outside.child("large.bin").write_binary(&vec![1; 32 * 1024]).expect("outside file exists");
        symlink(outside.path(), temp.child("linked").path()).expect("symlink exists");

        let allocations = path_allocations(temp.path()).expect("directory is measured");
        let outside_identity = FileIdentity {
            device: outside.child("large.bin").metadata().expect("metadata exists").dev(),
            inode: outside.child("large.bin").metadata().expect("metadata exists").ino(),
        };

        assert!(!allocations.iter().any(|allocation| allocation.identity == outside_identity));
    }
}
