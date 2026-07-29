use std::path::PathBuf;
use std::process::Command;

use rayon::prelude::*;

use crate::error::AppError;
use crate::footprint::{Estimate, Index, RootId};
use crate::fs::remove::{RemovalStatus, remove_file, safe_remove_dir_all};

use super::action::EntryKind;
use super::plan::{PathRemoval, ProcessRemoval, RemovalPlan};

#[derive(Debug)]
pub enum PathStatus {
    Removed,
    AlreadyAbsent,
    Retained,
    Failed(AppError),
}

#[derive(Debug)]
pub enum ProcessStatus {
    Completed,
    Failed(AppError),
}

#[derive(Debug)]
pub enum ActionOutcome {
    Path { path: PathBuf, status: PathStatus },
    Process { label: &'static str, program: &'static str, status: ProcessStatus },
}

#[derive(Debug)]
pub struct ApplyReport {
    outcomes: Vec<ActionOutcome>,
    freed_estimate: Estimate,
    estimation_error: Option<AppError>,
}

impl ApplyReport {
    pub fn outcomes(&self) -> &[ActionOutcome] {
        &self.outcomes
    }

    pub const fn freed_estimate(&self) -> Estimate {
        self.freed_estimate
    }

    pub fn removed_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| {
                matches!(
                    outcome,
                    ActionOutcome::Path { status: PathStatus::Removed, .. }
                        | ActionOutcome::Process { status: ProcessStatus::Completed, .. }
                )
            })
            .count()
    }

    pub fn absent_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| {
                matches!(outcome, ActionOutcome::Path { status: PathStatus::AlreadyAbsent, .. })
            })
            .count()
    }

    pub fn retained_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| {
                matches!(outcome, ActionOutcome::Path { status: PathStatus::Retained, .. })
            })
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| {
                matches!(
                    outcome,
                    ActionOutcome::Path { status: PathStatus::Failed(_), .. }
                        | ActionOutcome::Process { status: ProcessStatus::Failed(_), .. }
                )
            })
            .count()
            + usize::from(self.estimation_error.is_some())
    }

    pub fn planned_count(&self) -> usize {
        self.outcomes.len()
    }

    pub fn estimation_error(&self) -> Option<&AppError> {
        self.estimation_error.as_ref()
    }

    pub fn is_complete(&self) -> bool {
        self.retained_count() == 0 && self.failed_count() == 0
    }
}

pub fn apply_plan<P, F>(
    plan: &RemovalPlan,
    footprint: &Index,
    on_planned: P,
    on_completed: F,
) -> ApplyReport
where
    P: FnOnce(usize),
    F: Fn() + Sync,
{
    on_planned(plan.action_count());

    let path_outcomes = apply_paths(plan.paths(), &on_completed);
    let process_outcomes = apply_processes(plan.processes(), &on_completed);
    let completed_roots = path_outcomes.iter().filter_map(|(root, outcome)| {
        matches!(outcome, ActionOutcome::Path { status: PathStatus::Removed, .. }).then_some(*root)
    });
    let completed_estimates = process_outcomes.iter().filter_map(|(estimate, outcome)| {
        matches!(outcome, ActionOutcome::Process { status: ProcessStatus::Completed, .. })
            .then_some(*estimate)
    });
    let (freed_estimate, estimation_error) =
        match footprint.breakdown(completed_roots, completed_estimates) {
            Ok(breakdown) => (breakdown.total(), None),
            Err(error) => (Estimate::ZERO, Some(AppError::Footprint(error))),
        };
    let outcomes = path_outcomes
        .into_iter()
        .map(|(_, outcome)| outcome)
        .chain(process_outcomes.into_iter().map(|(_, outcome)| outcome))
        .collect();

    ApplyReport { outcomes, freed_estimate, estimation_error }
}

fn apply_paths<F>(paths: &[PathRemoval], on_completed: &F) -> Vec<(RootId, ActionOutcome)>
where
    F: Fn() + Sync,
{
    paths
        .par_iter()
        .map(|removal| {
            let result = match removal.kind() {
                EntryKind::File | EntryKind::Symlink => remove_file(removal.path()),
                EntryKind::Directory => safe_remove_dir_all(removal.path()),
            };
            on_completed();
            let status = match result {
                Ok(RemovalStatus::Removed) => PathStatus::Removed,
                Ok(RemovalStatus::AlreadyAbsent) => PathStatus::AlreadyAbsent,
                Ok(RemovalStatus::Retained) => PathStatus::Retained,
                Err(error) => PathStatus::Failed(error),
            };
            (removal.root(), ActionOutcome::Path { path: removal.path().to_path_buf(), status })
        })
        .collect()
}

fn apply_processes<F>(
    processes: &[ProcessRemoval],
    on_completed: &F,
) -> Vec<(Estimate, ActionOutcome)>
where
    F: Fn() + Sync,
{
    processes
        .iter()
        .map(|process| {
            let result = Command::new(process.program())
                .args(process.args())
                .status()
                .map_err(|source| AppError::ProcessStart {
                    label: process.label(),
                    program: process.program(),
                    source,
                })
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(AppError::ProcessExit {
                            label: process.label(),
                            program: process.program(),
                            status,
                        })
                    }
                });
            on_completed();
            let status = match result {
                Ok(()) => ProcessStatus::Completed,
                Err(error) => ProcessStatus::Failed(error),
            };
            (
                process.estimate(),
                ActionOutcome::Process {
                    label: process.label(),
                    program: process.program(),
                    status,
                },
            )
        })
        .collect()
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

        let report = apply_plan(&plan, &footprint, |_| {}, || {});

        directory.assert(predicates::path::missing());
        file.assert(predicates::path::missing());
        assert_eq!(report.removed_count(), 2);
        assert_eq!(report.failed_count(), 0);
        assert!(report.is_complete());
    }

    #[test]
    fn missing_paths_are_idempotent() {
        let temp = TempDir::new().expect("temp directory is created");
        let missing = temp.path().join("missing");
        let candidates = vec![Candidate::directory(TARGET, missing.clone())];
        let (plan, footprint) = prepare(&candidates);

        let report = apply_plan(&plan, &footprint, |_| {}, || {});

        assert!(!missing.exists());
        assert_eq!(report.removed_count(), 0);
        assert_eq!(report.absent_count(), 1);
        assert_eq!(report.failed_count(), 0);
        assert_eq!(report.freed_estimate(), Estimate::ZERO);
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

        let report = apply_plan(
            &plan,
            &footprint,
            |count| planned.store(count, Ordering::Relaxed),
            || {
                completed.fetch_add(1, Ordering::Relaxed);
            },
        );

        parent.assert(predicates::path::missing());
        assert_eq!(report.removed_count(), 1);
        assert_eq!(report.failed_count(), 0);
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

        let report = apply_plan(&plan, &footprint, |_| {}, || {});

        assert_eq!(report.freed_estimate().bytes(), expected);
    }

    #[test]
    fn terminal_symlink_candidate_removes_only_the_link_entry() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp directory is created");
        let physical = temp.child("physical");
        physical.create_dir_all().expect("physical directory exists");
        physical.child("cache.bin").write_binary(&[1; 4096]).expect("file exists");
        let alias = temp.child("alias");
        symlink(physical.path(), alias.path()).expect("symbolic link exists");
        let candidates = vec![Candidate::symlink(TARGET, alias.path().to_path_buf())];
        let (plan, footprint) = prepare(&candidates);
        let expected = footprint
            .breakdown(plan.roots(), plan.reported())
            .expect("footprint is available")
            .total();

        let report = apply_plan(&plan, &footprint, |_| {}, || {});

        physical.assert(predicates::path::is_dir());
        physical.child("cache.bin").assert(predicates::path::is_file());
        alias.assert(predicates::path::missing());
        assert_eq!(report.freed_estimate(), expected);
    }

    #[test]
    fn path_and_process_failures_remain_available_together() {
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

        let report = apply_plan(
            &plan,
            &footprint,
            |count| planned.store(count, Ordering::Relaxed),
            || {
                completed.fetch_add(1, Ordering::Relaxed);
            },
        );

        assert_eq!(report.failed_count(), 2);
        assert!(!report.is_complete());
        assert!(report.outcomes().iter().any(|outcome| matches!(
            outcome,
            ActionOutcome::Path { status: PathStatus::Failed(error), .. }
                if error.to_string().contains("remove file")
        )));
        assert!(report.outcomes().iter().any(|outcome| matches!(
            outcome,
            ActionOutcome::Process { status: ProcessStatus::Failed(error), .. }
                if error.to_string().contains("status")
        )));
        assert_eq!(std::fs::read_to_string(marker).expect("process recorded"), "invoked");
        assert_eq!(planned.load(Ordering::Relaxed), 2);
        assert_eq!(completed.load(Ordering::Relaxed), 2);
    }
}
