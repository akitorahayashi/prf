use std::io::{self, Write};

use crate::cleanup::{ScanReport, Target};
use crate::error::AppError;

use super::bytes::format_bytes;

pub fn prompt_for_targets<'a>(
    report: &ScanReport,
    available_targets: &[&'a Target],
) -> Result<Vec<&'a Target>, AppError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(
        output,
        "Select targets to delete (comma separated names or numbers). Type 'all' to select everything or press Enter to cancel."
    )?;

    for (index, target) in available_targets.iter().enumerate() {
        let size = report.standalone_estimate(target.id())?.bytes();
        writeln!(output, "  [{}] {:<8} {:>10}", index + 1, target, format_bytes(size))?;
    }

    write!(output, "Selection: ")?;
    output.flush()?;
    drop(output);

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    parse_selection(&input, available_targets)
}

pub fn parse_selection<'a>(
    input: &str,
    available: &[&'a Target],
) -> Result<Vec<&'a Target>, AppError> {
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

        if let Some(target) = available
            .iter()
            .copied()
            .find(|target| target.id().as_str().eq_ignore_ascii_case(token))
        {
            push_unique(&mut selected, target);
            continue;
        }

        if token.chars().all(|character| character.is_ascii_digit()) {
            let index = token
                .parse::<usize>()
                .map_err(|_| AppError::TargetIndexOutOfRange(token.to_string()))?;
            if index < 1 || index > available.len() {
                return Err(AppError::TargetIndexOutOfRange(token.to_string()));
            }
            push_unique(&mut selected, available[index - 1]);
            continue;
        }

        return Err(AppError::InvalidTarget(token.to_string()));
    }

    if selected.is_empty() {
        return Err(AppError::Cancelled);
    }

    Ok(selected)
}

fn push_unique<'a>(selected: &mut Vec<&'a Target>, target: &'a Target) {
    if !selected.iter().any(|existing| existing.id() == target.id()) {
        selected.push(target);
    }
}

pub fn confirm_deletion(total_size: u64) -> Result<bool, AppError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "About to delete {}. Proceed? [y/N]", format_bytes(total_size))?;
    write!(output, "Confirm: ")?;
    output.flush()?;
    drop(output);

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let answer = input.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup::{Discovery, ScopeSupport, TargetId};
    use crate::targets::registry;

    static TOOL2: Target = Target::new(
        TargetId::new("tool2"),
        "Tool 2",
        ScopeSupport::AllModes,
        Discovery::Rules(&[]),
    );

    fn available() -> Vec<&'static Target> {
        ["xcode", "python", "rust"]
            .iter()
            .map(|name| registry::find(name).expect("registered target"))
            .collect()
    }

    fn ids(targets: &[&Target]) -> Vec<&'static str> {
        targets.iter().map(|target| target.id().as_str()).collect()
    }

    #[test]
    fn parses_indices() {
        let available = available();
        let selected = parse_selection("1,3", &available).expect("valid indices");
        assert_eq!(ids(&selected), vec!["xcode", "rust"]);
    }

    #[test]
    fn parses_names() {
        let available = available();
        let selected = parse_selection("python,rust", &available).expect("valid names");
        assert_eq!(ids(&selected), vec!["python", "rust"]);
    }

    #[test]
    fn parses_mixed_names_and_indices() {
        let available = available();
        let selected = parse_selection("1,rust", &available).expect("valid mix");
        assert_eq!(ids(&selected), vec!["xcode", "rust"]);
    }

    #[test]
    fn collapses_duplicates() {
        let available = available();
        let selected = parse_selection("1,xcode,1", &available).expect("valid duplicates");
        assert_eq!(ids(&selected), vec!["xcode"]);
    }

    #[test]
    fn all_selects_everything_case_insensitively() {
        let available = available();
        assert_eq!(ids(&parse_selection("ALL", &available).expect("all")), ids(&available));
    }

    #[test]
    fn blank_input_cancels() {
        assert!(matches!(parse_selection("   ", &available()), Err(AppError::Cancelled)));
    }

    #[test]
    fn only_separators_cancel() {
        assert!(matches!(parse_selection(", ,", &available()), Err(AppError::Cancelled)));
    }

    #[test]
    fn out_of_range_index_is_rejected() {
        assert!(matches!(
            parse_selection("9", &available()),
            Err(AppError::TargetIndexOutOfRange(_))
        ));
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(matches!(parse_selection("java", &available()), Err(AppError::InvalidTarget(_))));
    }

    #[test]
    fn registered_but_unavailable_name_is_rejected() {
        assert!(matches!(parse_selection("docker", &available()), Err(AppError::InvalidTarget(_))));
    }

    #[test]
    fn digit_alpha_token_is_rejected() {
        assert!(matches!(parse_selection("1a", &available()), Err(AppError::InvalidTarget(_))));
    }

    #[test]
    fn registered_digit_name_is_resolved_before_index_parsing() {
        let available = vec![&TOOL2];

        assert_eq!(
            ids(&parse_selection("TOOL2", &available).expect("digit name resolves")),
            vec!["tool2"]
        );
    }
}
