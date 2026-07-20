use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dirs_next as dirs;

use crate::report::ScanReport;
use crate::targets::category::Category;
use crate::targets::item::{CleanupAction, CleanupItem};

use super::bytes::format_bytes;

pub fn display_path(path: &Path) -> String {
    if let Some(stripped) = dirs::home_dir().and_then(|home| path.strip_prefix(&home).ok()) {
        let mut display = PathBuf::from("~");
        display.push(stripped);
        return display.display().to_string();
    }

    path.display().to_string()
}

fn item_display(item: &CleanupItem) -> String {
    match &item.action {
        CleanupAction::Path { path, .. } => display_path(path),
        CleanupAction::DockerPrune => "Docker reclaimable (docker system prune)".to_string(),
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

pub fn deletion_summary(
    freed: u64,
    deleted_items: usize,
    skipped: usize,
    categories: usize,
) -> String {
    let mut summary = format!(
        "Reclaimed {} across {} {}: {} {} removed",
        format_bytes(freed),
        categories,
        plural(categories, "category", "categories"),
        deleted_items,
        plural(deleted_items, "item", "items"),
    );
    if skipped > 0 {
        summary.push_str(&format!(
            ", {} {} skipped (not empty after cleanup)",
            skipped,
            plural(skipped, "directory", "directories"),
        ));
    }
    summary.push('.');
    summary
}

pub fn print_scan_report(report: &ScanReport, categories: &[Category], verbose: bool) {
    println!("Scan results:");
    for category in categories {
        if let Some(category_report) = report.report_for(*category) {
            let total = category_report.total_size();
            println!(
                "- {:<8} {:>10} across {} item(s)",
                category.display_name(),
                format_bytes(total),
                category_report.items.len()
            );
            if verbose {
                for item in &category_report.items {
                    println!("    • {:<60} {}", item_display(item), format_bytes(item.size));
                }
            }
        }
    }
    println!("Total reclaimable: {}", format_bytes(report.total_size()));
}

pub fn print_list_results(results: &BTreeMap<Category, Vec<String>>) {
    println!("Found cleanup targets:");
    for (category, targets) in results {
        if !targets.is_empty() {
            println!("【{}】", category.display_name());
            for target in targets {
                println!("- {}", target);
            }
            println!();
        }
    }
}

pub fn print_deletion_plan(report: &ScanReport, categories: &[Category], verbose: bool) {
    println!("Deletion plan:");
    for category in categories {
        if let Some(category_report) = report.report_for(*category) {
            println!(
                "- {:<8} {:>10} across {} item(s)",
                category.display_name(),
                format_bytes(category_report.total_size()),
                category_report.items.len()
            );
            for item in &category_report.items {
                if verbose {
                    println!("    • {:<60} {}", item_display(item), format_bytes(item.size));
                } else {
                    println!("    • {}", item_display(item));
                }
            }
        }
    }
    println!("Total to delete: {}", format_bytes(report.total_size()));
}
