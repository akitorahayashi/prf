use std::collections::HashSet;

use crate::cleanup::{ScopeMode, Target, TargetId};
use crate::error::AppError;

use super::{brew, bun, docker, mise, nodejs, pnpm, python, rust, xcode};

static TARGETS: [&Target; 9] = [
    &xcode::TARGET,
    &python::TARGET,
    &rust::TARGET,
    &nodejs::TARGET,
    &mise::TARGET,
    &bun::TARGET,
    &pnpm::TARGET,
    &brew::TARGET,
    &docker::TARGET,
];

pub fn all() -> &'static [&'static Target] {
    &TARGETS
}

pub fn names() -> Vec<&'static str> {
    all().iter().map(|target| target.id().as_str()).collect()
}

pub fn find(name: &str) -> Option<&'static Target> {
    TARGETS.iter().copied().find(|target| target.id().as_str().eq_ignore_ascii_case(name))
}

pub fn eligible(mode: ScopeMode) -> Result<Vec<&'static Target>, AppError> {
    validate()?;

    Ok(TARGETS.iter().copied().filter(|target| target.scope_support().supports(mode)).collect())
}

pub fn resolve(names: &[String], mode: ScopeMode) -> Result<Vec<&'static Target>, AppError> {
    validate()?;

    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    for name in names {
        let target = find(name).ok_or_else(|| AppError::InvalidTarget(name.clone()))?;
        if seen.insert(target.id()) {
            selected.push(target);
        }
    }

    if mode == ScopeMode::Current {
        let unsupported: Vec<&str> = selected
            .iter()
            .filter(|target| !target.scope_support().supports(mode))
            .map(|target| target.id().as_str())
            .collect();
        if !unsupported.is_empty() {
            return Err(AppError::UnsupportedCurrentModeTarget(unsupported.join(", ")));
        }
    }

    Ok(selected)
}

fn validate() -> Result<(), AppError> {
    validate_targets(all())
}

fn validate_targets(targets: &[&Target]) -> Result<(), AppError> {
    let mut identifiers = HashSet::new();
    for target in targets {
        let id = target.id().as_str();
        if id == "all"
            || id.is_empty()
            || !id.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
        {
            return Err(AppError::InvalidTargetRegistry(format!(
                "invalid target identifier '{id}'"
            )));
        }
        if target.display_name().trim().is_empty() {
            return Err(AppError::InvalidTargetRegistry(format!(
                "target '{id}' has no display name"
            )));
        }
        if !identifiers.insert(TargetId::new(id)) {
            return Err(AppError::InvalidTargetRegistry(format!(
                "duplicate target identifier '{id}'"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup::ScopeSupport;

    #[test]
    fn registered_definitions_are_valid() {
        validate().expect("registry is valid");
    }

    #[test]
    fn explicit_selection_resolves_case_insensitively_and_deduplicates() {
        let selected = resolve(&["PYTHON".to_string(), "python".to_string()], ScopeMode::Default)
            .expect("selection resolves");

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id().as_str(), "python");
    }

    #[test]
    fn explicit_selection_preserves_first_occurrence_order() {
        let selected = resolve(
            &["rust".to_string(), "python".to_string(), "rust".to_string()],
            ScopeMode::Default,
        )
        .expect("selection resolves");

        assert_eq!(
            selected.iter().map(|target| target.id().as_str()).collect::<Vec<_>>(),
            vec!["rust", "python"]
        );
    }

    #[test]
    fn current_defaults_derive_from_registered_scope_support() {
        let selected = eligible(ScopeMode::Current).expect("current defaults resolve");

        assert!(selected.iter().all(|target| target.scope_support() == ScopeSupport::AllModes));
    }

    #[test]
    fn explicit_default_only_target_is_rejected_in_current_mode() {
        assert!(matches!(
            resolve(&["docker".to_string()], ScopeMode::Current),
            Err(AppError::UnsupportedCurrentModeTarget(_))
        ));
    }

    #[test]
    fn every_registered_name_resolves_case_insensitively() {
        for name in names() {
            let selected = resolve(&[name.to_ascii_uppercase()], ScopeMode::Default)
                .expect("registered name resolves");
            assert_eq!(selected[0].id().as_str(), name);
        }
    }

    #[test]
    fn registry_reserves_the_interactive_all_keyword() {
        static ALL: Target = Target::new(
            TargetId::new("all"),
            "All",
            ScopeSupport::AllModes,
            crate::cleanup::Discovery::Rules(&[]),
        );

        assert!(matches!(
            validate_targets(&[&ALL]),
            Err(AppError::InvalidTargetRegistry(message))
                if message.contains("invalid target identifier")
        ));
    }
}
