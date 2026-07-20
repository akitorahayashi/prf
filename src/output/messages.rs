//! Construction of individual user-facing message lines. Owning the wording here keeps
//! the app layer responsible for control flow only, not phrasing or layout.

use crate::targets::category::Category;

use super::bytes::format_bytes;

pub fn discovering(category: Category) -> String {
    format!("Discovering targets... ({})", category.display_name())
}

pub fn discovery_complete(category: Category, count: usize) -> String {
    format!(
        "✔︎ {} discovery complete ({} item{})",
        category.display_name(),
        count,
        if count == 1 { "" } else { "s" }
    )
}

pub fn size_calculation_complete(count: usize) -> String {
    format!(
        "{}/{} Size calculation complete ({} item{})",
        count,
        count,
        count,
        if count == 1 { "" } else { "s" }
    )
}

pub fn deletion_complete(count: usize) -> String {
    format!("{}/{} Deletion complete", count, count)
}

pub fn nothing_to_delete() -> &'static str {
    "Nothing to delete. All selected categories are already clean."
}

pub fn aborted() -> &'static str {
    "Aborted. No files were deleted."
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

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
