use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dirs_next as dirs;

use crate::targets::category::Category;
use crate::targets::item::CleanupAction;
use crate::targets::report::{CategoryStatus, ScanReport};

use super::bytes::format_bytes;

pub fn display_path(path: &Path) -> String {
    if let Some(stripped) = dirs::home_dir().and_then(|home| path.strip_prefix(&home).ok()) {
        let mut display = PathBuf::from("~");
        display.push(stripped);
        return display.display().to_string();
    }

    path.display().to_string()
}

pub fn print_scan_report(report: &ScanReport, categories: &[Category], verbose: bool) {
    println!("Scan results:");
    for category in categories {
        let Some(category_report) = report.report_for(*category) else {
            continue;
        };
        match &category_report.status {
            CategoryStatus::Ready => {
                let items = report.items_for_categories(&[*category]);
                println!(
                    "- {:<8} {:>10} across {} item(s)",
                    category.display_name(),
                    format_bytes(report.category_total_size(*category)),
                    items.len()
                );
                if verbose {
                    for item in items {
                        println!("    - {}", item_label(&item.action));
                    }
                }
            }
            CategoryStatus::Clean => {
                println!("- {:<8} clean", category.display_name());
            }
            CategoryStatus::Unavailable(reason) => {
                println!("- {:<8} unavailable: {reason}", category.display_name());
            }
            CategoryStatus::Failed(reason) => {
                println!("- {:<8} failed: {reason}", category.display_name());
            }
        }
    }
    println!("Total reclaimable: {}", format_bytes(report.total_size()));
}

pub fn print_list_results(report: &ScanReport, categories: &[Category]) {
    println!("Found cleanup targets:");
    for category in categories {
        let Some(category_report) = report.report_for(*category) else {
            continue;
        };
        println!("[{}]", category.display_name());
        match &category_report.status {
            CategoryStatus::Ready => {
                let mut counts = BTreeMap::new();
                for item in report.items_for_categories(&[*category]) {
                    *counts.entry(item_type_label(&item.action)).or_insert(0usize) += 1;
                }
                for (target, count) in counts {
                    println!(
                        "- {target} ({count} location{} found)",
                        if count == 1 { "" } else { "s" }
                    );
                }
            }
            CategoryStatus::Clean => println!("- clean"),
            CategoryStatus::Unavailable(reason) => println!("- unavailable: {reason}"),
            CategoryStatus::Failed(reason) => println!("- failed: {reason}"),
        }
        println!();
    }
}

pub fn print_deletion_plan(report: &ScanReport, categories: &[Category], verbose: bool) {
    println!("Deletion plan:");
    for category in categories {
        let Some(category_report) = report.report_for(*category) else {
            continue;
        };
        if category_report.status != CategoryStatus::Ready {
            continue;
        }
        println!(
            "- {:<8} {:>10} across {} item(s)",
            category.display_name(),
            format_bytes(report.category_total_size(*category)),
            report.category_item_count(*category)
        );
    }

    for item in report.items_for_categories(categories) {
        if verbose {
            println!("    - {:<60} {}", item_label(&item.action), format_bytes(item.size));
        } else {
            println!("    - {}", item_label(&item.action));
        }
    }
    println!("Total to delete: {}", format_bytes(report.total_size()));
}

fn item_label(action: &CleanupAction) -> String {
    match action {
        CleanupAction::Filesystem(candidate) => display_path(&candidate.path),
        CleanupAction::External(action) => action.description().to_string(),
    }
}

fn item_type_label(action: &CleanupAction) -> String {
    match action {
        CleanupAction::Filesystem(candidate) => candidate
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| display_path(&candidate.path)),
        CleanupAction::External(action) => action.description().to_string(),
    }
}
