use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::remove::{remove_file, safe_remove_dir_all};

use super::action::{Action, EntryKind};
use super::candidate::Candidate;

#[derive(Clone)]
struct PathRemoval {
    path: PathBuf,
    kind: EntryKind,
    size: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplySummary {
    pub applied: usize,
    pub failed: usize,
    pub freed_estimate: u64,
}

impl ApplySummary {
    fn merge(self, other: Self) -> Self {
        Self {
            applied: self.applied + other.applied,
            failed: self.failed + other.failed,
            freed_estimate: self.freed_estimate.saturating_add(other.freed_estimate),
        }
    }
}

fn is_strict_descendant(child: &Path, ancestor: &Path) -> bool {
    child != ancestor && child.starts_with(ancestor)
}

pub fn apply_candidates<P, F>(
    candidates: &[Candidate],
    on_planned: P,
    on_completed: F,
) -> Result<ApplySummary, AppError>
where
    P: FnOnce(usize),
    F: Fn() + Sync,
{
    let path_plan = prepare_root_paths(candidates);
    let process_count = candidates
        .iter()
        .filter(|candidate| matches!(&candidate.action, Action::RunProcess { .. }))
        .count();
    let action_count =
        path_plan.as_ref().map_or(process_count, |paths| paths.len() + process_count);
    on_planned(action_count);

    let path_result = path_plan.and_then(|paths| apply_paths(&paths, &on_completed));
    let process_result = apply_processes(candidates, &on_completed);
    combine_results(path_result, process_result)
}

fn apply_paths<F>(paths: &[PathRemoval], on_completed: &F) -> Result<ApplySummary, AppError>
where
    F: Fn() + Sync,
{
    let outcomes: Vec<Result<(bool, u64), AppError>> = paths
        .par_iter()
        .map(|removal| {
            let result = match removal.kind {
                EntryKind::File => remove_file(&removal.path),
                EntryKind::Directory => safe_remove_dir_all(&removal.path),
            };
            on_completed();
            result?;
            Ok((!removal.path.exists(), removal.size))
        })
        .collect();

    let mut summary = ApplySummary::default();
    let mut errors = Vec::new();
    for outcome in outcomes {
        match outcome {
            Ok((true, size)) => {
                summary.applied += 1;
                summary.freed_estimate = summary.freed_estimate.saturating_add(size);
            }
            Ok((false, _)) => summary.failed += 1,
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() { Ok(summary) } else { Err(group_error("path actions", errors)) }
}

fn apply_processes<F>(candidates: &[Candidate], on_completed: &F) -> Result<ApplySummary, AppError>
where
    F: Fn() + Sync,
{
    let mut summary = ApplySummary::default();
    let mut errors = Vec::new();

    for candidate in candidates {
        let Action::RunProcess { label, program, args } = &candidate.action else {
            continue;
        };

        let result = Command::new(program)
            .args(args.iter().copied())
            .status()
            .map_err(|error| AppError::Cleanup(format!("{label} could not start: {error}")))
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(AppError::Cleanup(format!("{label} failed with status {status}")))
                }
            });
        on_completed();

        match result {
            Ok(()) => {
                summary.applied += 1;
                summary.freed_estimate =
                    summary.freed_estimate.saturating_add(candidate.estimated_size());
            }
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() { Ok(summary) } else { Err(group_error("process actions", errors)) }
}

fn group_error(group: &str, errors: Vec<AppError>) -> AppError {
    let details = errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");
    AppError::Cleanup(format!("{group} failed: {details}"))
}

fn combine_results(
    paths: Result<ApplySummary, AppError>,
    processes: Result<ApplySummary, AppError>,
) -> Result<ApplySummary, AppError> {
    match (paths, processes) {
        (Ok(paths), Ok(processes)) => Ok(paths.merge(processes)),
        (Err(error), Ok(_)) | (Ok(_), Err(error)) => Err(error),
        (Err(path_error), Err(process_error)) => Err(AppError::Cleanup(format!(
            "multiple action groups failed: paths: {path_error}; processes: {process_error}"
        ))),
    }
}

fn prepare_root_paths(candidates: &[Candidate]) -> Result<Vec<PathRemoval>, AppError> {
    let paths = prepare_paths(candidates)?;
    let roots = paths
        .iter()
        .filter(|candidate| {
            !paths.iter().any(|other| is_strict_descendant(&candidate.path, &other.path))
        })
        .cloned()
        .collect();
    Ok(roots)
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

        let summary = apply_candidates(&candidates, |_| {}, || {}).expect("cleanup succeeds");

        directory.assert(predicates::path::missing());
        file.assert(predicates::path::missing());
        assert_eq!(summary.applied, 2);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn missing_paths_are_idempotent() {
        let temp = TempDir::new().expect("temp directory is created");
        let missing = temp.path().join("missing");
        let candidates = vec![Candidate::directory(TARGET, missing.clone())];

        let summary =
            apply_candidates(&candidates, |_| {}, || {}).expect("missing path is tolerated");

        assert!(!missing.exists());
        assert_eq!(summary.applied, 1);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn nested_candidates_report_and_complete_one_root_action() {
        let temp = TempDir::new().expect("temp directory is created");
        let parent = temp.child("node_modules");
        let child = parent.child("pkg/__pycache__");
        child.create_dir_all().expect("nested directory exists");
        child.child("cache.pyc").write_str("cache").expect("cache exists");
        let candidates = vec![
            Candidate::directory(TARGET, parent.path().to_path_buf()),
            Candidate::directory(TARGET, child.path().to_path_buf()),
        ];
        let planned = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);

        let summary = apply_candidates(
            &candidates,
            |count| planned.store(count, Ordering::Relaxed),
            || {
                completed.fetch_add(1, Ordering::Relaxed);
            },
        )
        .expect("cleanup succeeds");

        parent.assert(predicates::path::missing());
        assert_eq!(summary.applied, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(planned.load(Ordering::Relaxed), 1);
        assert_eq!(completed.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn process_action_runs_and_failures_combine_when_path_removal_fails() {
        let temp = TempDir::new().expect("temp directory is created");
        let directory = temp.child("directory-passed-as-file");
        directory.create_dir_all().expect("directory exists");
        let script = temp.child("record.sh");
        script.write_str("printf invoked > \"$1\"\nexit 7\n").expect("script exists");
        let marker = temp.path().join("process-invoked");
        let script_arg: &'static str =
            Box::leak(script.path().to_string_lossy().into_owned().into_boxed_str());
        let marker_arg: &'static str =
            Box::leak(marker.to_string_lossy().into_owned().into_boxed_str());
        let args: &'static [&'static str] =
            Box::leak(vec![script_arg, marker_arg].into_boxed_slice());
        let candidates = vec![
            Candidate::file(TARGET, directory.path().to_path_buf()),
            Candidate::process(TARGET, "record process", "/bin/sh", args, 0),
        ];
        let planned = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);

        let result = apply_candidates(
            &candidates,
            |count| planned.store(count, Ordering::Relaxed),
            || {
                completed.fetch_add(1, Ordering::Relaxed);
            },
        );

        let error = result.expect_err("both action groups fail");
        assert!(error.to_string().contains("multiple action groups failed"));
        assert_eq!(std::fs::read_to_string(marker).expect("process recorded"), "invoked");
        assert_eq!(planned.load(Ordering::Relaxed), 2);
        assert_eq!(completed.load(Ordering::Relaxed), 2);
    }
}
