use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::remove::remove_item;
use crate::output::messages;
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
    let scope = ScanScope::new(options.roots, options.current, options.verbose);
    let progress = Arc::new(MultiProgress::new());
    let report = scan_categories(&options.categories, &scope, &progress)?;

    // catalog::resolve excludes Docker under --current, so the membership check is sufficient.
    let docker_requested_initially = options.categories.contains(&Category::Docker);
    if report.total_size() == 0 && !docker_requested_initially {
        println!("{}", messages::nothing_to_delete());
        return Ok(());
    }

    let selected_categories = if options.interactive {
        match prompt_for_categories(&report, &options.categories) {
            Ok(categories) => categories,
            Err(AppError::Cancelled) => {
                println!("{}", messages::aborted());
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
        println!("{}", messages::nothing_to_delete());
        return Ok(());
    }

    print_deletion_plan(&subset, &selected_categories, options.verbose);

    if !options.assume_yes && !confirm_deletion(subset.total_size())? {
        println!("{}", messages::aborted());
        return Ok(());
    }

    // The typed action carries the filesystem/command distinction: delete_items only ever
    // touches Path actions, and the Docker prune is routed to run_cleanup below.
    let items_to_delete = flatten_items_for_categories(&subset, &selected_categories);
    let fs_result = delete_items(&items_to_delete, &progress, options.verbose);

    let docker_result =
        if docker_selected { run_docker_cleanup_with_handling(options.verbose) } else { Ok(()) };

    let fs_summary = match (fs_result, docker_result) {
        (Ok(summary), Ok(())) => summary,
        (Err(err), Ok(())) | (Ok(_), Err(err)) => return Err(err),
        (Err(fs_err), Err(docker_err)) => {
            return Err(AppError::Io(io::Error::other(format!(
                "multiple cleanup failures: filesystem: {fs_err}; docker: {docker_err}"
            ))));
        }
    };

    // Docker cleanup ran (the match above only yields Ok when the prune succeeded), so its
    // item and reclaimable estimate count toward the outcome.
    let docker_report = subset.report_for(Category::Docker);
    let docker_items = docker_report.map_or(0, |report| report.items.len());
    let docker_freed = docker_report.map_or(0, |report| report.total_size());

    let removed_items = fs_summary.removed + docker_items;
    let freed_estimate = fs_summary.freed_estimate + docker_freed;
    let categories_with_items = subset.categories().len();
    println!(
        "{}",
        messages::deletion_summary(
            freed_estimate,
            removed_items,
            fs_summary.failed,
            categories_with_items
        )
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
    size: u64,
}

struct DeletionSummary {
    /// Targets whose path no longer exists after the deletion pass (verified outcome).
    removed: usize,
    /// Targets still present afterwards (e.g. a directory left non-empty).
    failed: usize,
    /// Scan-time size of the removed targets. An estimate, not a re-measurement: roots do
    /// not nest after pruning, so summing their sizes does not double-count.
    freed_estimate: u64,
}

fn is_strict_descendant(child: &Path, ancestor: &Path) -> bool {
    child != ancestor && child.starts_with(ancestor)
}

fn delete_items(
    items: &[CleanupItem],
    progress: &Arc<MultiProgress>,
    verbose: bool,
) -> Result<DeletionSummary, AppError> {
    // Canonicalize and collapse duplicates. A path that is a strict descendant of another
    // prepared path is subsumed by that ancestor's recursive deletion, so it is dropped here:
    // this removes all nesting up front, which is what makes the parallel deletion below
    // race-free (no depth ordering required).
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
        prepared.push(FsDeletion { path: canonicalized, kind: *kind, size: item.size });
    }

    let roots: Vec<&FsDeletion> = prepared
        .iter()
        .filter(|candidate| {
            !prepared.iter().any(|other| is_strict_descendant(&candidate.path, &other.path))
        })
        .collect();

    if roots.is_empty() {
        return Ok(DeletionSummary { removed: 0, failed: 0, freed_estimate: 0 });
    }

    let pb = progress.add(ProgressBar::new(roots.len() as u64));
    pb.set_style(deletion_progress_style());

    // A hard error aborts; otherwise a target counts as removed only if its path is gone
    // afterwards, so a skipped (still non-empty) directory is not reported as reclaimed.
    let outcomes: Result<Vec<(bool, u64)>, AppError> = roots
        .par_iter()
        .map(|deletion| {
            remove_item(&deletion.path, deletion.kind, verbose)?;
            let removed = !deletion.path.exists();
            pb.inc(1);
            Ok((removed, deletion.size))
        })
        .collect();
    let outcomes = outcomes?;

    pb.finish_and_clear();

    let removed = outcomes.iter().filter(|(removed, _)| *removed).count();
    let freed_estimate =
        outcomes.iter().filter(|(removed, _)| *removed).map(|(_, size)| size).sum();
    let failed = outcomes.len() - removed;

    let _ = progress.println(messages::deletion_complete(removed));
    Ok(DeletionSummary { removed, failed, freed_estimate })
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use indicatif::ProgressDrawTarget;

    use crate::targets::category::Category;
    use crate::targets::item::CleanupItem;

    use super::*;

    // The progress bar is an incidental dependency of delete_items, not the behavior under
    // test. A hidden draw target keeps its rendering and completion line off stderr so test
    // runs in a TTY do not leak progress frames.
    fn hidden_progress() -> Arc<MultiProgress> {
        Arc::new(MultiProgress::with_draw_target(ProgressDrawTarget::hidden()))
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
            CleanupItem::directory(Category::Nodejs, dir.path().to_path_buf(), 0),
            CleanupItem::file(Category::Nodejs, file.path().to_path_buf(), 0),
        ];

        let progress = hidden_progress();
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

        let progress = hidden_progress();
        delete_items(&items, &progress, false).expect("deletion succeeds even with missing item");

        dir.assert(predicates::path::missing());
        file.assert(predicates::path::missing());
    }

    #[test]
    fn delete_items_prunes_nested_child_of_selected_parent() {
        let temp = TempDir::new().expect("temp directory is created");
        let node_modules = temp.child("node_modules");
        node_modules.create_dir_all().expect("node_modules exists");
        let nested_pycache = node_modules.child("pkg/__pycache__");
        nested_pycache.create_dir_all().expect("nested pycache exists");
        nested_pycache.child("foo.pyc").write_str("cache").expect("cache file exists");

        // A python item nested inside a selected nodejs item: the parent's recursive
        // deletion subsumes the child, so only one deletion is attempted for the subtree.
        let items = vec![
            CleanupItem::directory(Category::Nodejs, node_modules.path().to_path_buf(), 0),
            CleanupItem::directory(Category::Python, nested_pycache.path().to_path_buf(), 0),
        ];

        let progress = hidden_progress();
        let summary = delete_items(&items, &progress, false).expect("deletion succeeds");

        node_modules.assert(predicates::path::missing());
        nested_pycache.assert(predicates::path::missing());
        assert_eq!(summary.removed, 1, "nested child is subsumed by the parent deletion");
        assert_eq!(summary.failed, 0, "no target should be left behind");
    }
}
