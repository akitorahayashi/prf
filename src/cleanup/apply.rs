use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::remove::{remove_file, safe_remove_dir_all};

use super::action::{Action, EntryKind};
use super::candidate::Candidate;

struct PathRemoval {
    path: PathBuf,
    kind: EntryKind,
    size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplySummary {
    pub applied: usize,
    pub failed: usize,
    pub freed_estimate: u64,
}

fn is_strict_descendant(child: &Path, ancestor: &Path) -> bool {
    child != ancestor && child.starts_with(ancestor)
}

pub fn apply_candidates<F>(
    candidates: &[Candidate],
    on_applied: F,
) -> Result<ApplySummary, AppError>
where
    F: Fn() + Sync,
{
    let paths = prepare_paths(candidates)?;
    let roots: Vec<&PathRemoval> = paths
        .iter()
        .filter(|candidate| {
            !paths.iter().any(|other| is_strict_descendant(&candidate.path, &other.path))
        })
        .collect();

    let outcomes: Result<Vec<(bool, u64)>, AppError> = roots
        .par_iter()
        .map(|removal| {
            match removal.kind {
                EntryKind::File => remove_file(&removal.path)?,
                EntryKind::Directory => safe_remove_dir_all(&removal.path)?,
            }
            let removed = !removal.path.exists();
            on_applied();
            Ok((removed, removal.size))
        })
        .collect();
    let outcomes = outcomes?;

    let mut applied = outcomes.iter().filter(|(removed, _)| *removed).count();
    let failed = outcomes.len() - applied;
    let mut freed_estimate: u64 =
        outcomes.iter().filter(|(removed, _)| *removed).map(|(_, size)| size).sum();

    for candidate in candidates {
        let Action::RunProcess { label, program, args } = &candidate.action else {
            continue;
        };

        let status = Command::new(program)
            .args(args.iter().copied())
            .status()
            .map_err(|error| AppError::Cleanup(format!("{label} could not start: {error}")))?;
        if !status.success() {
            return Err(AppError::Cleanup(format!("{label} failed with status {status}")));
        }
        applied += 1;
        freed_estimate = freed_estimate.saturating_add(candidate.estimated_size());
        on_applied();
    }

    Ok(ApplySummary { applied, failed, freed_estimate })
}

fn prepare_paths(candidates: &[Candidate]) -> Result<Vec<PathRemoval>, AppError> {
    let mut prepared: Vec<PathRemoval> = Vec::new();
    let mut seen: HashMap<PathBuf, usize> = HashMap::new();

    for candidate in candidates {
        let Action::RemovePath { path, kind } = &candidate.action else {
            continue;
        };

        let resolved = match std::fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) if error.kind() == ErrorKind::NotFound => path.clone(),
            Err(error) => return Err(AppError::Io(error)),
        };

        if let Some(index) = seen.get(&resolved).copied() {
            if prepared[index].kind != *kind {
                return Err(AppError::Cleanup(format!(
                    "conflicting entry kinds for {}",
                    resolved.display()
                )));
            }
            prepared[index].size = prepared[index].size.max(candidate.estimated_size());
            continue;
        }

        seen.insert(resolved.clone(), prepared.len());
        prepared.push(PathRemoval {
            path: resolved,
            kind: *kind,
            size: candidate.estimated_size(),
        });
    }

    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;
    use crate::cleanup::TargetId;

    const TARGET: TargetId = TargetId::new("test");

    #[test]
    fn removes_files_and_directories() {
        let temp = TempDir::new().expect("temp directory is created");
        let directory = temp.child("node_modules");
        directory.child("lib").create_dir_all().expect("directory exists");
        directory.child("lib/index.js").write_str("cache").expect("file exists");
        let file = temp.child("cache.log");
        file.write_str("hello").expect("file exists");
        let candidates = vec![
            Candidate::directory(TARGET, directory.path().to_path_buf()),
            Candidate::file(TARGET, file.path().to_path_buf()),
        ];
        let applied = AtomicUsize::new(0);

        let summary = apply_candidates(&candidates, || {
            applied.fetch_add(1, Ordering::Relaxed);
        })
        .expect("cleanup succeeds");

        directory.assert(predicates::path::missing());
        file.assert(predicates::path::missing());
        assert_eq!(summary.applied, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(applied.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn missing_paths_are_idempotent() {
        let temp = TempDir::new().expect("temp directory is created");
        let missing = temp.path().join("missing");
        let candidates = vec![Candidate::directory(TARGET, missing.clone())];

        let summary = apply_candidates(&candidates, || {}).expect("missing path is tolerated");

        assert!(!missing.exists());
        assert_eq!(summary.applied, 1);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn nested_candidates_collapse_to_the_selected_ancestor() {
        let temp = TempDir::new().expect("temp directory is created");
        let parent = temp.child("node_modules");
        let child = parent.child("pkg/__pycache__");
        child.create_dir_all().expect("nested directory exists");
        child.child("cache.pyc").write_str("cache").expect("cache exists");
        let candidates = vec![
            Candidate::directory(TARGET, parent.path().to_path_buf()),
            Candidate::directory(TARGET, child.path().to_path_buf()),
        ];

        let summary = apply_candidates(&candidates, || {}).expect("cleanup succeeds");

        parent.assert(predicates::path::missing());
        assert_eq!(summary.applied, 1);
        assert_eq!(summary.failed, 0);
    }
}
