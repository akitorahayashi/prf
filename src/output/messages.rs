//! Construction of individual user-facing message lines.

use crate::cleanup::EstimateSummary;

use super::bytes::format_bytes;

pub fn discovering(display_name: &str) -> String {
    format!("Discovering targets... ({display_name})")
}

pub fn discovery_complete(display_name: &str, count: usize) -> String {
    format!(
        "✔︎ {display_name} discovery complete ({} item{})",
        count,
        if count == 1 { "" } else { "s" }
    )
}

pub fn calculating_footprint(count: usize) -> String {
    format!("Calculating footprint... ({} item{})", count, if count == 1 { "" } else { "s" })
}

pub fn footprint_calculation_complete(count: usize) -> String {
    format!(
        "{count}/{count} Footprint calculation complete ({} item{})",
        count,
        if count == 1 { "" } else { "s" }
    )
}

pub fn deletion_complete(completed: usize, planned: usize) -> String {
    format!("{completed}/{planned} Cleanup actions attempted")
}

pub fn nothing_to_delete() -> &'static str {
    "No cleanup actions were discovered."
}

pub fn aborted() -> &'static str {
    "Aborted. No files were deleted."
}

pub fn deletion_summary(
    reclaimed: EstimateSummary,
    removed: usize,
    absent: usize,
    retained: usize,
    failed: usize,
    targets: usize,
) -> String {
    let reclaimed = match (reclaimed.known().bytes(), reclaimed.unestimated_actions()) {
        (known, 0) => format!("~{}", format_bytes(known)),
        (0, count) => {
            format!("an unestimated amount from {count} {}", plural(count, "action", "actions"))
        }
        (known, count) => format!(
            "~{} known plus {count} unestimated {}",
            format_bytes(known),
            plural(count, "action", "actions")
        ),
    };
    format!(
        "Reclaimed {reclaimed} across {} {}: {} completed, {} already absent, {} retained, {} failed.",
        targets,
        plural(targets, "target", "targets"),
        removed,
        absent,
        retained,
        failed,
    )
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
