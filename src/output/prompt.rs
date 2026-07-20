use std::io::{self, Write};

use crate::error::AppError;
use crate::report::ScanReport;
use crate::targets::category::Category;

use super::bytes::format_bytes;

pub fn prompt_for_categories(
    report: &ScanReport,
    available_categories: &[Category],
) -> Result<Vec<Category>, AppError> {
    println!(
        "Select categories to delete (comma separated names or numbers). Type 'all' to select everything or press Enter to cancel."
    );

    for (index, category) in available_categories.iter().enumerate() {
        let report = report.report_for(*category);
        let size = report.map(|value| value.total_size()).unwrap_or_default();
        println!("  [{}] {:<8} {:>10}", index + 1, category, format_bytes(size));
    }

    print!("Selection: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::Cancelled);
    }
    if trimmed.eq_ignore_ascii_case("all") {
        return Ok(available_categories.to_vec());
    }

    let mut selected = Vec::new();

    for token in trimmed.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        if let Ok(index) = token.parse::<usize>() {
            if index < 1 || index > available_categories.len() {
                return Err(AppError::CategoryIndexOutOfRange(token.to_string()));
            }

            let category = available_categories[index - 1];
            if !selected.contains(&category) {
                selected.push(category);
            }
            continue;
        }

        if token.chars().any(|ch| ch.is_ascii_digit())
            && token.chars().any(|ch| ch.is_ascii_alphabetic())
        {
            return Err(AppError::InvalidCategory(token.to_string()));
        }

        if let Some(category) =
            token.parse::<Category>().ok().filter(|c| available_categories.contains(c))
        {
            if !selected.contains(&category) {
                selected.push(category);
            }
            continue;
        }

        return Err(AppError::InvalidCategory(token.to_string()));
    }

    if selected.is_empty() {
        return Err(AppError::Cancelled);
    }

    Ok(selected)
}

pub fn confirm_deletion(total_size: u64) -> Result<bool, AppError> {
    println!("About to delete {}. Proceed? [y/N]", format_bytes(total_size));
    print!("Confirm: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let answer = input.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}
