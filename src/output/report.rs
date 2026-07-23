use std::io::{self, Write};
use std::path::{Path, PathBuf};

use dirs_next as dirs;

use crate::cleanup::{
    Action, ActionOutcome, ApplyReport, Inspection, Listing, PathStatus, ProcessStatus, ScanReport,
    Target,
};
use crate::error::AppError;

use super::bytes::format_bytes;
use super::messages;

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

pub fn print_stdout_line(message: &str) -> Result<(), AppError> {
    writeln!(io::stdout().lock(), "{message}")?;
    Ok(())
}

pub fn print_diagnostics(inspections: &[Inspection]) -> Result<(), AppError> {
    let stderr = io::stderr();
    let mut output = stderr.lock();
    for diagnostic in inspections.iter().flat_map(|inspection| &inspection.diagnostics) {
        writeln!(output, "Warning: {}", diagnostic.message)?;
    }
    Ok(())
}

pub fn print_scan_report(
    report: &ScanReport,
    targets: &[&Target],
    verbose: bool,
) -> Result<(), AppError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "Scan results:")?;
    for target in targets {
        if let Some(target_report) = report.report_for(target.id()) {
            writeln!(
                output,
                "- {:<8} {:>10} across {} item(s)",
                target.display_name(),
                format_bytes(target_report.estimate().bytes()),
                target_report.candidates.len()
            )?;
            if verbose {
                for candidate_report in &target_report.candidates {
                    writeln!(
                        output,
                        "    • {:<60} {}",
                        candidate_display(&candidate_report.candidate.action),
                        format_bytes(candidate_report.estimate().bytes())
                    )?;
                }
            }
        }
    }
    writeln!(output, "Total reclaimable: {}", format_bytes(report.estimate().bytes()))?;
    Ok(())
}

pub fn print_list_results(targets: &[&Target], inspections: &[Inspection]) -> Result<(), AppError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "Found cleanup targets:")?;
    for (target, inspection) in targets.iter().zip(inspections) {
        if inspection.listings.is_empty() {
            continue;
        }

        writeln!(output, "【{}】", target.display_name())?;
        for listing in &inspection.listings {
            match listing {
                Listing::Count { label, count } => writeln!(
                    output,
                    "- {} ({} location{} found)",
                    label,
                    count,
                    if *count == 1 { "" } else { "s" }
                )?,
                Listing::Path(path) => writeln!(output, "- {} (exists)", path.display())?,
                Listing::Detail(detail) => writeln!(output, "- {detail}")?,
            };
        }
        writeln!(output)?;
    }
    Ok(())
}

pub fn print_deletion_plan(
    report: &ScanReport,
    targets: &[&Target],
    verbose: bool,
) -> Result<(), AppError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "Deletion plan:")?;
    for target in targets {
        if let Some(target_report) = report.report_for(target.id()) {
            writeln!(
                output,
                "- {:<8} {:>10} across {} item(s)",
                target.display_name(),
                format_bytes(target_report.estimate().bytes()),
                target_report.candidates.len()
            )?;
            for candidate_report in &target_report.candidates {
                if verbose {
                    writeln!(
                        output,
                        "    • {:<60} {}",
                        candidate_display(&candidate_report.candidate.action),
                        format_bytes(candidate_report.estimate().bytes())
                    )?;
                } else {
                    writeln!(
                        output,
                        "    • {}",
                        candidate_display(&candidate_report.candidate.action)
                    )?;
                }
            }
        }
    }
    writeln!(output, "Total to delete: {}", format_bytes(report.estimate().bytes()))?;
    Ok(())
}

pub fn print_cleanup_report(report: &ApplyReport, targets: usize) -> Result<(), AppError> {
    {
        let stderr = io::stderr();
        let mut errors = stderr.lock();
        for outcome in report.outcomes() {
            match outcome {
                ActionOutcome::Path { path, status: PathStatus::Retained } => {
                    writeln!(
                        errors,
                        "Retained: {} (the directory was not empty after cleanup)",
                        display_path(path)
                    )?;
                }
                ActionOutcome::Path { path, status: PathStatus::Failed(error) } => {
                    writeln!(errors, "Failed: {}: {error}", display_path(path))?;
                }
                ActionOutcome::Process { label, program, status: ProcessStatus::Failed(error) } => {
                    writeln!(errors, "Failed: {label} via '{program}': {error}")?;
                }
                _ => {}
            }
        }
        if let Some(error) = report.estimation_error() {
            writeln!(errors, "Failed to calculate reclaimed footprint: {error}")?;
        }
    }

    print_stdout_line(&messages::deletion_summary(
        report.freed_estimate().bytes(),
        report.removed_count(),
        report.absent_count(),
        report.retained_count(),
        report.failed_count(),
        targets,
    ))
}
