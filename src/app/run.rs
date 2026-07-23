use std::path::PathBuf;
use std::sync::Arc;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::remove::remove_item;
use crate::output::bytes::format_bytes;
use crate::output::progress::deletion_progress_style;
use crate::output::prompt::{confirm_deletion, prompt_for_categories};
use crate::output::report::print_deletion_plan;
use crate::targets::catalog::RequestOrigin;
use crate::targets::category::Category;
use crate::targets::docker;
use crate::targets::item::{CleanupAction, CleanupItem, ExternalAction};
use crate::targets::report::ScanReport;
use crate::targets::target::ScanScope;

use super::scan::{scan_categories, validate_report};

pub struct RunOptions {
    pub categories: Vec<Category>,
    pub request_origin: RequestOrigin,
    pub interactive: bool,
    pub roots: Vec<PathBuf>,
    pub verbose: bool,
    pub assume_yes: bool,
    pub current: bool,
}

pub fn execute(options: RunOptions) -> Result<(), AppError> {
    let scope = ScanScope::new(options.roots, options.current, options.verbose);
    let progress = Arc::new(MultiProgress::new());
    let report = scan_categories(&options.categories, &scope, &progress, true);
    validate_report(&report, &options.categories, options.request_origin)?;

    if report.total_size() == 0 {
        println!("Nothing to delete. All selected categories are already clean.");
        return Ok(());
    }

    let selected_categories = if options.interactive {
        let ready_categories = report.ready_categories(&options.categories);
        match prompt_for_categories(&report, &ready_categories) {
            Ok(categories) => categories,
            Err(AppError::Cancelled) => {
                println!("Aborted. No files were deleted.");
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    } else {
        options.categories.clone()
    };

    let subset = report.subset(&selected_categories);
    if subset.total_size() == 0 {
        println!("Nothing to delete. All selected categories are already clean.");
        return Ok(());
    }

    print_deletion_plan(&subset, &selected_categories, options.verbose);
    if !options.assume_yes && !confirm_deletion(subset.total_size())? {
        println!("Aborted. No files were deleted.");
        return Ok(());
    }

    let items_to_delete = subset.items_for_categories(&selected_categories);
    let filesystem_items = items_to_delete
        .iter()
        .filter(|item| matches!(item.action, CleanupAction::Filesystem(_)))
        .cloned()
        .collect::<Vec<_>>();
    let docker_selected = items_to_delete
        .iter()
        .any(|item| item.external_action() == Some(ExternalAction::DockerPrune));

    let filesystem_result = delete_items(&filesystem_items, &progress, options.verbose);
    let docker_result = if docker_selected { docker::run_cleanup(options.verbose) } else { Ok(()) };

    match (filesystem_result, docker_result) {
        (Ok(()), Ok(())) => {}
        (Err(error), Ok(())) | (Ok(()), Err(error)) => return Err(error),
        (Err(filesystem_error), Err(docker_error)) => {
            return Err(AppError::ExternalCommand(format!(
                "multiple cleanup failures: filesystem: {filesystem_error}; docker: {docker_error}"
            )));
        }
    }

    println!(
        "Attempted to delete {} across {} categor(ies).",
        format_bytes(subset.total_size()),
        selected_categories.len()
    );
    Ok(())
}

fn delete_items(
    items: &[CleanupItem],
    progress: &Arc<MultiProgress>,
    verbose: bool,
) -> Result<(), AppError> {
    let filesystem_items =
        items.iter().filter_map(CleanupItem::filesystem_candidate).collect::<Vec<_>>();
    if filesystem_items.is_empty() {
        return Ok(());
    }

    let progress_bar = progress.add(ProgressBar::new(filesystem_items.len() as u64));
    progress_bar.set_style(deletion_progress_style());

    filesystem_items.par_iter().try_for_each(|item| {
        remove_item(&item.path, item.kind, verbose)?;
        progress_bar.inc(1);
        Ok::<(), AppError>(())
    })?;

    progress_bar.finish_and_clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use crate::targets::item::{ItemKind, PathAuthority};

    use super::*;

    fn candidate(
        category: Category,
        path: PathBuf,
        kind: ItemKind,
        authority: PathBuf,
    ) -> CleanupItem {
        CleanupItem::filesystem(category, path, kind, PathAuthority::LocalRoot(authority))
    }

    #[test]
    fn delete_items_removes_files_and_directories() {
        let temp = TempDir::new().expect("temp directory is created");
        let dir = temp.child("node_modules");
        dir.child("lib").create_dir_all().expect("directory exists");
        dir.child("lib/index.js").write_str("console.log('cache');").expect("file exists");
        let file = temp.child("cache.log");
        file.write_str("hello").expect("file exists");

        let items = vec![
            candidate(
                Category::Nodejs,
                dir.path().to_path_buf(),
                ItemKind::Directory,
                temp.path().to_path_buf(),
            ),
            candidate(
                Category::Nodejs,
                file.path().to_path_buf(),
                ItemKind::File,
                temp.path().to_path_buf(),
            ),
        ];

        delete_items(&items, &Arc::new(MultiProgress::new()), false).expect("deletion succeeds");

        dir.assert(predicates::path::missing());
        file.assert(predicates::path::missing());
    }

    #[test]
    fn delete_items_handles_already_deleted_targets_idempotently() {
        let temp = TempDir::new().expect("temp directory is created");
        let file = temp.child("cache.log");
        file.write_str("hello").expect("file exists");
        let items = vec![candidate(
            Category::Nodejs,
            file.path().to_path_buf(),
            ItemKind::File,
            temp.path().to_path_buf(),
        )];
        std::fs::remove_file(file.path()).expect("pre-delete file");

        delete_items(&items, &Arc::new(MultiProgress::new()), false)
            .expect("deletion succeeds with a missing item");

        file.assert(predicates::path::missing());
    }
}
