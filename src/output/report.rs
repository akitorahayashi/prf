use std::path::{Path, PathBuf};

use dirs_next as dirs;

use crate::cleanup::{Action, Inspection, Listing, ScanReport, Target};

use super::bytes::format_bytes;

pub fn display_path(path: &Path) -> String {
    if let Some(stripped) = dirs::home_dir().and_then(|home| path.strip_prefix(&home).ok()) {
        let mut display = PathBuf::from("~");
        display.push(stripped);
        return display.display().to_string();
    }

    path.display().to_string()
}

fn candidate_display(action: &Action) -> String {
    match action {
        Action::RemovePath { path, .. } => display_path(path),
        Action::RunProcess { label, .. } => (*label).to_string(),
    }
}

pub fn print_diagnostics(inspections: &[Inspection]) {
    for diagnostic in inspections.iter().flat_map(|inspection| &inspection.diagnostics) {
        eprintln!("Warning: {}", diagnostic.message);
    }
}

pub fn print_scan_report(report: &ScanReport, targets: &[&Target], verbose: bool) {
    println!("Scan results:");
    for target in targets {
        if let Some(target_report) = report.report_for(target.id()) {
            println!(
                "- {:<8} {:>10} across {} item(s)",
                target.display_name(),
                format_bytes(target_report.total_size()),
                target_report.candidates.len()
            );
            if verbose {
                for candidate in &target_report.candidates {
                    println!(
                        "    • {:<60} {}",
                        candidate_display(&candidate.action),
                        format_bytes(candidate.estimated_size())
                    );
                }
            }
        }
    }
    println!("Total reclaimable: {}", format_bytes(report.total_size()));
}

pub fn print_list_results(targets: &[&Target], inspections: &[Inspection]) {
    println!("Found cleanup targets:");
    for (target, inspection) in targets.iter().zip(inspections) {
        if inspection.listings.is_empty() {
            continue;
        }

        println!("【{}】", target.display_name());
        for listing in &inspection.listings {
            match listing {
                Listing::Count { label, count } => println!(
                    "- {} ({} location{} found)",
                    label,
                    count,
                    if *count == 1 { "" } else { "s" }
                ),
                Listing::Path(path) => println!("- {} (exists)", path.display()),
                Listing::Detail(detail) => println!("- {detail}"),
            }
        }
        println!();
    }
}

pub fn print_deletion_plan(report: &ScanReport, targets: &[&Target], verbose: bool) {
    println!("Deletion plan:");
    for target in targets {
        if let Some(target_report) = report.report_for(target.id()) {
            println!(
                "- {:<8} {:>10} across {} item(s)",
                target.display_name(),
                format_bytes(target_report.total_size()),
                target_report.candidates.len()
            );
            for candidate in &target_report.candidates {
                if verbose {
                    println!(
                        "    • {:<60} {}",
                        candidate_display(&candidate.action),
                        format_bytes(candidate.estimated_size())
                    );
                } else {
                    println!("    • {}", candidate_display(&candidate.action));
                }
            }
        }
    }
    println!("Total to delete: {}", format_bytes(report.total_size()));
}
