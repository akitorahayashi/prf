use std::process::Command;

use rayon::prelude::*;

use crate::error::AppError;
use crate::footprint::{Estimate, Index, RootId};
use crate::fs::remove::{remove_file, safe_remove_dir_all};

use super::action::EntryKind;
use super::plan::{PathRemoval, ProcessRemoval, RemovalPlan};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplySummary {
    pub applied: usize,
    pub failed: usize,
    pub freed_estimate: Estimate,
}

struct PathOutcome {
    applied: usize,
    failed: usize,
    roots: Vec<RootId>,
}

struct ProcessOutcome {
    applied: usize,
    estimates: Vec<Estimate>,
}

pub fn apply_plan<P, F>(
    plan: &RemovalPlan,
    footprint: &Index,
    on_planned: P,
    on_completed: F,
) -> Result<ApplySummary, AppError>
where
    P: FnOnce(usize),
    F: Fn() + Sync,
{
    on_planned(plan.action_count());

    let paths = apply_paths(plan.paths(), &on_completed);
    let processes = apply_processes(plan.processes(), &on_completed);

    match (paths, processes) {
        (Ok(paths), Ok(processes)) => {
            let footprint = footprint.breakdown(paths.roots, processes.estimates)?;
            Ok(ApplySummary {
                applied: paths.applied + processes.applied,
                failed: paths.failed,
                freed_estimate: footprint.total(),
            })
        }
        (Err(error), Ok(_)) | (Ok(_), Err(error)) => Err(error),
        (Err(path_error), Err(process_error)) => Err(AppError::Cleanup(format!(
            "multiple action groups failed: paths: {path_error}; processes: {process_error}"
        ))),
    }
}

fn apply_paths<F>(paths: &[PathRemoval], on_completed: &F) -> Result<PathOutcome, AppError>
where
    F: Fn() + Sync,
{
    let outcomes: Vec<Result<(bool, RootId), AppError>> = paths
        .par_iter()
        .map(|removal| {
            let result = match removal.kind() {
                EntryKind::File => remove_file(removal.path()),
                EntryKind::Directory => safe_remove_dir_all(removal.path()),
            };
            on_completed();
            result?;
            Ok((!removal.path().try_exists()?, removal.root()))
        })
        .collect();

    let mut applied = 0;
    let mut failed = 0;
    let mut roots = Vec::new();
    let mut errors = Vec::new();
    for outcome in outcomes {
        match outcome {
            Ok((true, root)) => {
                applied += 1;
                roots.push(root);
            }
            Ok((false, _)) => failed += 1,
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() {
        Ok(PathOutcome { applied, failed, roots })
    } else {
        Err(group_error("path actions", errors))
    }
}

fn apply_processes<F>(
    processes: &[ProcessRemoval],
    on_completed: &F,
) -> Result<ProcessOutcome, AppError>
where
    F: Fn() + Sync,
{
    let mut applied = 0;
    let mut estimates = Vec::new();
    let mut errors = Vec::new();

    for process in processes {
        let result = Command::new(process.program())
            .args(process.args())
            .status()
            .map_err(|error| {
                AppError::Cleanup(format!("{} could not start: {error}", process.label()))
            })
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(AppError::Cleanup(format!(
                        "{} failed with status {status}",
                        process.label()
                    )))
                }
            });
        on_completed();

        match result {
            Ok(()) => {
                applied += 1;
                estimates.push(process.estimate());
            }
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() {
        Ok(ProcessOutcome { applied, estimates })
    } else {
        Err(group_error("process actions", errors))
    }
}

fn group_error(group: &str, errors: Vec<AppError>) -> AppError {
    let details = errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");
    AppError::Cleanup(format!("{group} failed: {details}"))
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;
    use crate::cleanup::{Candidate, RemovalCatalog, TargetId};

    const TARGET: TargetId = TargetId::new("test");

    fn prepare(candidates: &[Candidate]) -> (RemovalPlan, Index) {
        let catalog = RemovalCatalog::new(candidates.to_vec()).expect("catalog is valid");
        let footprint =
            Index::measure(&catalog.measurement_roots()).expect("footprint is measured");
        let selected = (0..candidates.len()).collect::<Vec<_>>();
        let plan = catalog.plan(&selected).expect("plan is built");
        (plan, footprint)
    }

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
        let (plan, footprint) = prepare(&candidates);

        let summary = apply_plan(&plan, &footprint, |_| {}, || {}).expect("cleanup succeeds");

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
        let (plan, footprint) = prepare(&candidates);

        let summary =
            apply_plan(&plan, &footprint, |_| {}, || {}).expect("missing path is tolerated");

        assert!(!missing.exists());
        assert_eq!(summary.applied, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.freed_estimate, Estimate::ZERO);
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
        let (plan, footprint) = prepare(&candidates);
        let planned = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);

        let summary = apply_plan(
            &plan,
            &footprint,
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
    fn successful_summary_uses_selection_aware_hard_link_estimate() {
        let temp = TempDir::new().expect("temp directory is created");
        let first_root = temp.child("first");
        let second_root = temp.child("second");
        first_root.create_dir_all().expect("first root exists");
        second_root.create_dir_all().expect("second root exists");
        let first = first_root.child("shared.bin");
        first.write_binary(&[1; 4096]).expect("file exists");
        fs::hard_link(first.path(), second_root.path().join("shared.bin"))
            .expect("hard link exists");
        let expected = fs::metadata(first_root.path()).unwrap().blocks() * 512
            + fs::metadata(second_root.path()).unwrap().blocks() * 512
            + fs::metadata(first.path()).unwrap().blocks() * 512;
        let candidates = vec![
            Candidate::directory(TARGET, first_root.path().to_path_buf()),
            Candidate::directory(TARGET, second_root.path().to_path_buf()),
        ];
        let (plan, footprint) = prepare(&candidates);

        let summary = apply_plan(&plan, &footprint, |_| {}, || {}).expect("cleanup succeeds");

        assert_eq!(summary.freed_estimate.bytes(), expected);
    }

    #[test]
    fn linked_candidate_root_uses_the_same_physical_path_for_measurement_and_removal() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp directory is created");
        let physical = temp.child("physical");
        physical.create_dir_all().expect("physical directory exists");
        physical.child("cache.bin").write_binary(&[1; 4096]).expect("file exists");
        let alias = temp.child("alias");
        symlink(physical.path(), alias.path()).expect("symbolic link exists");
        let candidates = vec![Candidate::directory(TARGET, alias.path().to_path_buf())];
        let (plan, footprint) = prepare(&candidates);
        let expected = footprint
            .breakdown(plan.roots(), plan.reported())
            .expect("footprint is available")
            .total();

        let summary = apply_plan(&plan, &footprint, |_| {}, || {}).expect("cleanup succeeds");

        physical.assert(predicates::path::missing());
        assert!(fs::symlink_metadata(alias.path()).unwrap().file_type().is_symlink());
        assert_eq!(summary.freed_estimate, expected);
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
        let (plan, footprint) = prepare(&candidates);
        let planned = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);

        let result = apply_plan(
            &plan,
            &footprint,
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
