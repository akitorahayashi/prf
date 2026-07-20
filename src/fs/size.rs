use std::path::Path;

use walkdir::WalkDir;

/// Best-effort byte size of a file or directory tree.
///
/// A path that has vanished between discovery and measurement (a benign race with a
/// concurrent build) contributes 0, with a verbose-mode note. Sizing is non-destructive,
/// so a missing path is tolerated rather than surfaced as a hard error.
pub fn path_size(path: &Path, verbose: bool) -> u64 {
    let metadata = match path.metadata() {
        Ok(metadata) => metadata,
        Err(err) => {
            if verbose {
                eprintln!("Skipping {}: {}", path.display(), err);
            }
            return 0;
        }
    };

    if metadata.is_file() {
        return metadata.len();
    }

    let mut total = 0u64;
    for entry in WalkDir::new(path).into_iter() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                if verbose {
                    eprintln!("Skipping {:?}: {}", err.path(), err);
                }
                continue;
            }
        };

        if entry.file_type().is_file() {
            match entry.metadata() {
                Ok(metadata) => {
                    total = total.saturating_add(metadata.len());
                }
                Err(err) => {
                    if verbose {
                        eprintln!("Skipping {}: {}", entry.path().display(), err);
                    }
                }
            }
        }
    }
    total
}
