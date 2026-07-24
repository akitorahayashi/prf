//! Construction of individual user-facing message lines.

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
    freed: u64,
    removed: usize,
    absent: usize,
    retained: usize,
    failed: usize,
    targets: usize,
) -> String {
    format!(
        "Reclaimed ~{} across {} {}: {} completed, {} already absent, {} retained, {} failed.",
        format_bytes(freed),
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
