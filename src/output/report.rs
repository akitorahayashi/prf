use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::cleanup::report::{CandidateReport, TargetReport};
use crate::cleanup::{
    Action, ActionOutcome, ApplyReport, Inspection, Listing, PathStatus, ProcessStatus, ScanReport,
    Target,
};
use crate::error::AppError;

use super::bytes::{format_action_estimate, format_estimate_compact, format_estimate_detail};
use super::messages;

pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(stripped) = home.and_then(|home| path.strip_prefix(home).ok()) {
        let mut display = PathBuf::from("~");
        display.push(stripped);
        return display.display().to_string();
    }

    path.display().to_string()
}

fn candidate_display<'a>(action: &'a Action, home: Option<&Path>) -> Cow<'a, str> {
    match action {
        Action::RemovePath { path, .. } => Cow::Owned(display_path(path, home)),
        Action::RunProcess { label, .. } => Cow::Borrowed(label),
    }
}

fn write_target_summary(
    output: &mut impl Write,
    target: &Target,
    report: &TargetReport,
) -> io::Result<()> {
    writeln!(
        output,
        "- {:<8} {:>10} across {} item(s)",
        target.display_name(),
        format_estimate_compact(report.estimate()),
        report.candidates.len()
    )
}

fn write_candidate(
    output: &mut impl Write,
    report: &CandidateReport,
    home: Option<&Path>,
) -> io::Result<()> {
    writeln!(output, "    • {}", candidate_display(report.candidate.action(), home))
}

fn write_candidate_with_estimate(
    output: &mut impl Write,
    report: &CandidateReport,
    home: Option<&Path>,
) -> io::Result<()> {
    writeln!(
        output,
        "    • {:<60} {}",
        candidate_display(report.candidate.action(), home),
        format_action_estimate(report.estimate())
    )
}

pub fn print_stdout_line(message: &str) -> Result<(), AppError> {
    writeln!(io::stdout().lock(), "{message}")?;
    Ok(())
}

pub fn print_diagnostics(inspections: &[Inspection]) -> Result<(), AppError> {
    let stderr = io::stderr();
    let mut output = stderr.lock();
    let mut rendered = HashSet::new();
    for diagnostic in inspections.iter().flat_map(|inspection| &inspection.diagnostics) {
        if rendered.insert(diagnostic) {
            writeln!(output, "Warning: {diagnostic}")?;
        }
    }
    Ok(())
}

pub fn print_scan_report(
    report: &ScanReport,
    targets: &[&Target],
    verbose: bool,
    home: Option<&Path>,
) -> Result<(), AppError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "Scan results:")?;
    for target in targets {
        if let Some(target_report) = report.report_for(target.id()) {
            write_target_summary(&mut output, target, target_report)?;
            if verbose {
                for candidate_report in &target_report.candidates {
                    write_candidate_with_estimate(&mut output, candidate_report, home)?;
                }
            }
        }
    }
    writeln!(output, "Total reclaimable: {}", format_estimate_detail(report.estimate()))?;
    Ok(())
}

pub fn print_list_results(
    targets: &[&Target],
    inspections: &[Inspection],
    home: Option<&Path>,
) -> Result<(), AppError> {
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
                Listing::Path(path) => writeln!(output, "- {} (exists)", display_path(path, home))?,
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
    home: Option<&Path>,
) -> Result<(), AppError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "Deletion plan:")?;
    for target in targets {
        if let Some(target_report) = report.report_for(target.id()) {
            write_target_summary(&mut output, target, target_report)?;
            for candidate_report in &target_report.candidates {
                if verbose {
                    write_candidate_with_estimate(&mut output, candidate_report, home)?;
                } else {
                    write_candidate(&mut output, candidate_report, home)?;
                }
            }
        }
    }
    writeln!(output, "Total to delete: {}", format_estimate_detail(report.estimate()))?;
    Ok(())
}

pub fn print_cleanup_report(
    report: &ApplyReport,
    targets: usize,
    home: Option<&Path>,
) -> Result<(), AppError> {
    {
        let stderr = io::stderr();
        let mut errors = stderr.lock();
        for outcome in report.outcomes() {
            match outcome {
                ActionOutcome::Path { path, status: PathStatus::Retained } => {
                    writeln!(
                        errors,
                        "Retained: {} (the directory was not empty after cleanup)",
                        display_path(path, home)
                    )?;
                }
                ActionOutcome::Path { path, status: PathStatus::Failed(error) } => {
                    writeln!(errors, "Failed: {}: {error}", display_path(path, home))?;
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
        report.reclaimed(),
        report.removed_count(),
        report.absent_count(),
        report.retained_count(),
        report.failed_count(),
        targets,
    ))
}
