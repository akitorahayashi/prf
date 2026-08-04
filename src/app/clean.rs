use std::sync::Arc;

use indicatif::{MultiProgress, ProgressBar};

use crate::cleanup::{Scope, Target, apply_plan};
use crate::error::AppError;
use crate::output::messages;
use crate::output::progress::deletion_progress_style;
use crate::output::prompt::{confirm_deletion, prompt_for_targets};
use crate::output::report::{print_cleanup_report, print_deletion_plan, print_stdout_line};

use super::scan::scan_targets;

pub enum CleanSelection {
    PromptFrom(Vec<&'static Target>),
    Fixed(Vec<&'static Target>),
}

pub struct CleanOptions {
    pub selection: CleanSelection,
    pub scope: Scope,
    pub verbose: bool,
    pub assume_yes: bool,
}

trait DecisionSource {
    fn select_targets<'a>(
        &mut self,
        report: &crate::cleanup::ScanReport,
        available: &[&'a Target],
    ) -> Result<Vec<&'a Target>, AppError>;

    fn confirm_deletion(&mut self, total_size: u64) -> Result<bool, AppError>;
}

struct TerminalDecisions;

impl DecisionSource for TerminalDecisions {
    fn select_targets<'a>(
        &mut self,
        report: &crate::cleanup::ScanReport,
        available: &[&'a Target],
    ) -> Result<Vec<&'a Target>, AppError> {
        prompt_for_targets(report, available)
    }

    fn confirm_deletion(&mut self, total_size: u64) -> Result<bool, AppError> {
        confirm_deletion(total_size)
    }
}

pub fn execute(options: CleanOptions) -> Result<(), AppError> {
    execute_with(options, &mut TerminalDecisions)
}

fn execute_with(
    options: CleanOptions,
    decisions: &mut impl DecisionSource,
) -> Result<(), AppError> {
    let (targets, prompt_for_selection) = match options.selection {
        CleanSelection::PromptFrom(targets) => (targets, true),
        CleanSelection::Fixed(targets) => (targets, false),
    };
    let progress = Arc::new(MultiProgress::new());
    let report = scan_targets(&targets, &options.scope, &progress)?;

    if report.is_empty() {
        print_stdout_line(messages::nothing_to_delete())?;
        return Ok(());
    }

    let selected_targets = if prompt_for_selection {
        match decisions.select_targets(&report, &targets) {
            Ok(targets) => targets,
            Err(AppError::Cancelled) => {
                print_stdout_line(messages::aborted())?;
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    } else {
        targets
    };

    let subset = report.subset(&selected_targets)?;
    if subset.is_empty() {
        print_stdout_line(messages::nothing_to_delete())?;
        return Ok(());
    }

    print_deletion_plan(&subset, &selected_targets, options.verbose, options.scope.home())?;
    if !options.assume_yes && !decisions.confirm_deletion(subset.estimate().bytes())? {
        print_stdout_line(messages::aborted())?;
        return Ok(());
    }

    let plan = subset.removal_plan();
    let deletion_bar = progress.add(ProgressBar::new(0));
    deletion_bar.set_style(deletion_progress_style());
    let report = apply_plan(
        plan,
        subset.footprint(),
        |count| deletion_bar.set_length(count as u64),
        || deletion_bar.inc(1),
    );
    deletion_bar.finish_and_clear();

    progress.println(messages::deletion_complete(report.planned_count(), plan.action_count()))?;
    print_cleanup_report(&report, subset.target_ids().len(), options.scope.home())?;
    if report.is_complete() {
        Ok(())
    } else {
        Err(AppError::IncompleteCleanup {
            retained: report.retained_count(),
            failed: report.failed_count(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::registry;
    use tempfile::TempDir;

    struct RecordedDecisions {
        selections: usize,
        confirmations: usize,
        confirmed: bool,
        selected_target: Option<crate::cleanup::TargetId>,
    }

    impl DecisionSource for RecordedDecisions {
        fn select_targets<'a>(
            &mut self,
            _report: &crate::cleanup::ScanReport,
            available: &[&'a Target],
        ) -> Result<Vec<&'a Target>, AppError> {
            self.selections += 1;
            Ok(available
                .iter()
                .copied()
                .filter(|target| {
                    self.selected_target.is_none_or(|selected| target.id() == selected)
                })
                .collect())
        }

        fn confirm_deletion(&mut self, _total_size: u64) -> Result<bool, AppError> {
            self.confirmations += 1;
            Ok(self.confirmed)
        }
    }

    #[test]
    fn omitted_targets_prompt_then_apply_only_the_selected_report_subset() {
        let directory = TempDir::new().expect("temporary directory is created");
        let cache = directory.path().join("project/__pycache__");
        std::fs::create_dir_all(&cache).expect("cache directory is created");
        std::fs::write(cache.join("module.pyc"), "cache").expect("cache file is created");
        let rust_project = directory.path().join("crate");
        let rust_cache = rust_project.join("target");
        std::fs::create_dir_all(&rust_cache).expect("Rust target directory is created");
        std::fs::write(rust_project.join("Cargo.toml"), "[package]")
            .expect("Rust manifest is created");
        let scope = Scope::resolve(true, None, directory.path().to_path_buf())
            .expect("current scope resolves");
        let python = registry::find("python").expect("python target is registered");
        let rust = registry::find("rust").expect("Rust target is registered");
        let mut decisions = RecordedDecisions {
            selections: 0,
            confirmations: 0,
            confirmed: true,
            selected_target: Some(python.id()),
        };

        execute_with(
            CleanOptions {
                selection: CleanSelection::PromptFrom(vec![python, rust]),
                scope,
                verbose: false,
                assume_yes: false,
            },
            &mut decisions,
        )
        .expect("approved cleanup succeeds");

        assert_eq!(decisions.selections, 1);
        assert_eq!(decisions.confirmations, 1);
        assert!(!cache.exists());
        assert!(rust_cache.exists());
    }

    #[test]
    fn rejected_confirmation_preserves_the_selected_report_subset() {
        let directory = TempDir::new().expect("temporary directory is created");
        let cache = directory.path().join("project/__pycache__");
        std::fs::create_dir_all(&cache).expect("cache directory is created");
        std::fs::write(cache.join("module.pyc"), "cache").expect("cache file is created");
        let scope = Scope::resolve(true, None, directory.path().to_path_buf())
            .expect("current scope resolves");
        let python = registry::find("python").expect("python target is registered");
        let mut decisions = RecordedDecisions {
            selections: 0,
            confirmations: 0,
            confirmed: false,
            selected_target: None,
        };

        execute_with(
            CleanOptions {
                selection: CleanSelection::Fixed(vec![python]),
                scope,
                verbose: false,
                assume_yes: false,
            },
            &mut decisions,
        )
        .expect("rejected cleanup exits successfully");

        assert_eq!(decisions.selections, 0);
        assert_eq!(decisions.confirmations, 1);
        assert!(cache.exists());
    }
}
