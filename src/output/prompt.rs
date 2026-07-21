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

    parse_selection(&input, available_categories)
}

/// Parses an interactive selection line into categories. Accepts 1-based indices, category
/// names, and `all`; blank input (or only separators) cancels. A token mixing digits and
/// letters is rejected rather than partially matched.
pub fn parse_selection(input: &str, available: &[Category]) -> Result<Vec<Category>, AppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::Cancelled);
    }
    if trimmed.eq_ignore_ascii_case("all") {
        return Ok(available.to_vec());
    }

    let mut selected = Vec::new();
    for token in trimmed.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        if let Ok(index) = token.parse::<usize>() {
            if index < 1 || index > available.len() {
                return Err(AppError::CategoryIndexOutOfRange(token.to_string()));
            }
            push_unique(&mut selected, available[index - 1]);
            continue;
        }

        if token.chars().any(|ch| ch.is_ascii_digit())
            && token.chars().any(|ch| ch.is_ascii_alphabetic())
        {
            return Err(AppError::InvalidCategory(token.to_string()));
        }

        match Category::from_name(token) {
            Some(category) if available.contains(&category) => push_unique(&mut selected, category),
            _ => return Err(AppError::InvalidCategory(token.to_string())),
        }
    }

    if selected.is_empty() {
        return Err(AppError::Cancelled);
    }

    Ok(selected)
}

fn push_unique(selected: &mut Vec<Category>, category: Category) {
    if !selected.contains(&category) {
        selected.push(category);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const AVAILABLE: [Category; 3] = [Category::Xcode, Category::Python, Category::Rust];

    #[test]
    fn parses_indices() {
        assert_eq!(
            parse_selection("1,3", &AVAILABLE).expect("valid indices"),
            vec![Category::Xcode, Category::Rust]
        );
    }

    #[test]
    fn parses_names() {
        assert_eq!(
            parse_selection("python,rust", &AVAILABLE).expect("valid names"),
            vec![Category::Python, Category::Rust]
        );
    }

    #[test]
    fn parses_mixed_names_and_indices() {
        assert_eq!(
            parse_selection("1,rust", &AVAILABLE).expect("valid mix"),
            vec![Category::Xcode, Category::Rust]
        );
    }

    #[test]
    fn collapses_duplicates() {
        assert_eq!(
            parse_selection("1,xcode,1", &AVAILABLE).expect("valid duplicates"),
            vec![Category::Xcode]
        );
    }

    #[test]
    fn all_selects_everything_case_insensitively() {
        assert_eq!(parse_selection("ALL", &AVAILABLE).expect("all"), AVAILABLE.to_vec());
    }

    #[test]
    fn blank_input_cancels() {
        assert!(matches!(parse_selection("   ", &AVAILABLE), Err(AppError::Cancelled)));
    }

    #[test]
    fn only_separators_cancels() {
        assert!(matches!(parse_selection(", ,", &AVAILABLE), Err(AppError::Cancelled)));
    }

    #[test]
    fn out_of_range_index_is_rejected() {
        assert!(matches!(
            parse_selection("9", &AVAILABLE),
            Err(AppError::CategoryIndexOutOfRange(_))
        ));
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(matches!(parse_selection("java", &AVAILABLE), Err(AppError::InvalidCategory(_))));
    }

    #[test]
    fn known_name_not_available_is_rejected() {
        assert!(matches!(parse_selection("docker", &AVAILABLE), Err(AppError::InvalidCategory(_))));
    }

    #[test]
    fn digit_alpha_token_is_rejected() {
        assert!(matches!(parse_selection("1a", &AVAILABLE), Err(AppError::InvalidCategory(_))));
    }
}
