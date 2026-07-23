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

pub fn size_calculation_complete(count: usize) -> String {
    format!(
        "{count}/{count} Size calculation complete ({} item{})",
        count,
        if count == 1 { "" } else { "s" }
    )
}

pub fn deletion_complete(count: usize) -> String {
    format!("{count}/{count} Deletion complete")
}

pub fn nothing_to_delete() -> &'static str {
    "Nothing to delete. All selected targets are already clean."
}

pub fn aborted() -> &'static str {
    "Aborted. No files were deleted."
}

pub fn deletion_summary(freed: u64, applied: usize, failed: usize, targets: usize) -> String {
    let mut summary = format!(
        "Reclaimed ~{} across {} {}: {} {} removed",
        format_bytes(freed),
        targets,
        plural(targets, "target", "targets"),
        applied,
        plural(applied, "item", "items"),
    );
    if failed > 0 {
        summary.push_str(&format!(
            ", {} {} could not be removed (not empty after cleanup)",
            failed,
            plural(failed, "item", "items"),
        ));
    }
    summary.push('.');
    summary
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
