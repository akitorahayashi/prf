use std::collections::HashMap;
use std::io;
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
use crate::report::ScanReport;
use crate::targets::category::Category;
use crate::targets::docker;
use crate::targets::item::{CleanupAction, CleanupItem, ItemKind};
use crate::targets::target::ScanScope;

use super::scan::scan_categories;

pub struct RunOptions {
    pub categories: Vec<Category>,
    pub interactive: bool,
    pub roots: Vec<PathBuf>,
    pub verbose: bool,
    pub assume_yes: bool,
    pub current: bool,
}

pub fn execute(options: RunOptions) -> Result<(), AppError> {
    let debug_logging = std::env::var_os("PRF_DEBUG").is_some();

    let scope = ScanScope::new(options.roots, options.current, options.verbose);
    let progress = Arc::new(MultiProgress::new());
    let report = scan_categories(&options.categories, &scope, &progress)?;

    if debug_logging {
        eprintln!("[prf::run] finished scan phase");
    }

    // catalog::resolve excludes Docker under --current, so the membership check is sufficient.
    let docker_requested_initially = options.categories.contains(&Category::Docker);
    if report.total_size() == 0 && !docker_requested_initially {
        println!("Nothing to delete. All selected categories are already clean.");
        return Ok(());
    }

    let selected_categories = if options.interactive {
        match prompt_for_categories(&report, &options.categories) {
            Ok(categories) => categories,
            Err(AppError::Cancelled) => {
                println!("Aborted. No files were deleted.");
                return Ok(());
            }
            Err(err) => return Err(err),
        }
    } else {
        options.categories.clone()
    };

    let docker_selected = selected_categories.contains(&Category::Docker);
    let subset = report.subset(&selected_categories);
    if subset.total_size() == 0 && !docker_selected {
        println!("Nothing to delete. All selected categories are already clean.");
        return Ok(());
    }

    print_deletion_plan(&subset, &selected_categories, options.verbose);

    if debug_logging {
        eprintln!("[prf::run] printed summary, awaiting confirmation");
    }

    if !options.assume_yes && !confirm_deletion(subset.total_size())? {
        println!("Aborted. No files were deleted.");
        return Ok(());
    }

    if debug_logging {
        eprintln!("[prf::run] confirmation obtained");
    }

    // The typed action carries the filesystem/command distinction: delete_items only ever
    // touches Path actions, and the Docker prune is routed to run_cleanup below.
    let items_to_delete = flatten_items_for_categories(&subset, &selected_categories);
    let fs_result = delete_items(&items_to_delete, &progress, options.verbose);

    let docker_result =
        if docker_selected { run_docker_cleanup_with_handling(options.verbose) } else { Ok(()) };

    match (fs_result, docker_result) {
        (Ok(()), Ok(())) => {}
        (Err(err), Ok(())) | (Ok(()), Err(err)) => return Err(err),
        (Err(fs_err), Err(docker_err)) => {
            return Err(AppError::Io(io::Error::other(format!(
                "multiple cleanup failures: filesystem: {fs_err}; docker: {docker_err}"
            ))));
        }
    }

    if debug_logging {
        eprintln!("[prf::run] deletion phase complete");
    }

    println!(
        "Attempted to delete {} across {} categor(ies).",
        format_bytes(subset.total_size()),
        selected_categories.len()
    );

    Ok(())
}

fn flatten_items_for_categories(report: &ScanReport, categories: &[Category]) -> Vec<CleanupItem> {
    categories
        .iter()
        .filter_map(|category| report.report_for(*category))
        .flat_map(|category_report| category_report.items.clone())
        .collect()
}

fn run_docker_cleanup_with_handling(verbose: bool) -> Result<(), AppError> {
    match docker::run_cleanup(verbose) {
        Ok(()) => Ok(()),
        Err(AppError::Io(err)) if err.kind() == io::ErrorKind::NotFound => {
            if verbose {
                eprintln!("Docker CLI not available; skipping Docker cleanup.");
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

struct FsDeletion {
    path: PathBuf,
    kind: ItemKind,
}

fn delete_items(
    items: &[CleanupItem],
    progress: &Arc<MultiProgress>,
    verbose: bool,
) -> Result<(), AppError> {
    let mut prepared: Vec<FsDeletion> = Vec::new();
    let mut seen_paths: HashMap<String, usize> = HashMap::new();

    for item in items {
        let CleanupAction::Path { path, kind } = &item.action else {
            continue;
        };

        let canonicalized = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        let key = canonicalized.to_string_lossy().into_owned();

        if let Some(index) = seen_paths.get(&key).copied() {
            if prepared[index].kind != *kind {
                prepared[index].kind = ItemKind::Directory;
            }
            continue;
        }

        seen_paths.insert(key, prepared.len());
        prepared.push(FsDeletion { path: canonicalized, kind: *kind });
    }

    if prepared.is_empty() {
        return Ok(());
    }

    prepared.sort_by_key(|deletion| std::cmp::Reverse(deletion.path.components().count()));

    let pb = progress.add(ProgressBar::new(prepared.len() as u64));
    pb.set_style(deletion_progress_style());

    prepared.par_iter().try_for_each(|deletion| {
        remove_item(&deletion.path, deletion.kind, verbose)?;
        pb.inc(1);
        Ok::<(), AppError>(())
    })?;

    pb.finish_and_clear();
    let _ = progress.println(format!("{}/{} Deletion complete", prepared.len(), prepared.len()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use crate::targets::category::Category;
    use crate::targets::item::CleanupItem;

    use super::*;

    #[test]
    fn delete_items_removes_files_and_directories() {
        let temp = TempDir::new().expect("temp directory is created");
        let dir = temp.child("node_modules");
        dir.child("lib").create_dir_all().expect("directory exists");
        dir.child("lib/index.js").write_str("console.log('cache');").expect("file exists");
        let file = temp.child("cache.log");
        file.write_str("hello").expect("file exists");

        let items = vec![
            CleanupItem::directory(Category::Nodejs, dir.path().to_path_buf(), 0),
            CleanupItem::file(Category::Nodejs, file.path().to_path_buf(), 0),
        ];

        let progress = Arc::new(MultiProgress::new());
        delete_items(&items, &progress, false).expect("deletion succeeds");

        dir.assert(predicates::path::missing());
        file.assert(predicates::path::missing());
    }

    #[test]
    fn delete_items_handles_already_deleted_targets_idempotently() {
        let temp = TempDir::new().expect("temp directory is created");
        let dir = temp.child("node_modules");
        dir.child("lib").create_dir_all().expect("directory exists");
        dir.child("lib/index.js").write_str("console.log('cache');").expect("file exists");
        let file = temp.child("cache.log");
        file.write_str("hello").expect("file exists");

        let items = vec![
            CleanupItem::directory(Category::Nodejs, dir.path().to_path_buf(), 0),
            CleanupItem::file(Category::Nodejs, file.path().to_path_buf(), 0),
        ];

        std::fs::remove_file(file.path()).expect("pre-delete file");

        let progress = Arc::new(MultiProgress::new());
        delete_items(&items, &progress, false).expect("deletion succeeds even with missing item");

        dir.assert(predicates::path::missing());
        file.assert(predicates::path::missing());
    }
}
