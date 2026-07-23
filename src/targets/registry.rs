use std::collections::HashSet;

use crate::cleanup::{ScopeMode, Target, TargetId};
use crate::error::AppError;

use super::{brew, docker, nodejs, python, rust, xcode};

static TARGETS: [&Target; 6] = [
    &xcode::TARGET,
    &python::TARGET,
    &rust::TARGET,
    &nodejs::TARGET,
    &brew::TARGET,
    &docker::TARGET,
];

pub fn all() -> &'static [&'static Target] {
    &TARGETS
}

pub fn find(name: &str) -> Option<&'static Target> {
    TARGETS.iter().copied().find(|target| target.id().as_str().eq_ignore_ascii_case(name))
}

pub fn resolve(
    names: &[String],
    all_requested: bool,
    mode: ScopeMode,
) -> Result<Vec<&'static Target>, AppError> {
    validate()?;

    let selected = if all_requested || names.is_empty() {
        TARGETS
            .iter()
            .copied()
            .filter(|target| {
                mode != ScopeMode::Current || target.scope_support().supports_current()
            })
            .collect()
    } else {
        let mut seen = HashSet::new();
        let mut selected = Vec::new();
        for name in names {
            let target = find(name).ok_or_else(|| AppError::InvalidTarget(name.clone()))?;
            if seen.insert(target.id()) {
                selected.push(target);
            }
        }
        selected
    };

    if mode == ScopeMode::Current {
        let unsupported: Vec<&str> = selected
            .iter()
            .filter(|target| !target.scope_support().supports_current())
            .map(|target| target.id().as_str())
            .collect();
        if !unsupported.is_empty() {
            return Err(AppError::UnsupportedCurrentModeTarget(unsupported.join(", ")));
        }
    }

    Ok(selected)
}

fn validate() -> Result<(), AppError> {
    let mut identifiers = HashSet::new();
    for target in all() {
        let id = target.id().as_str();
        if id.is_empty()
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
        let selected =
            resolve(&["PYTHON".to_string(), "python".to_string()], false, ScopeMode::Default)
                .expect("selection resolves");

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id().as_str(), "python");
    }

    #[test]
    fn current_defaults_derive_from_registered_scope_support() {
        let selected = resolve(&[], false, ScopeMode::Current).expect("current defaults resolve");

        assert!(selected.iter().all(|target| target.scope_support() == ScopeSupport::AllModes));
    }

    #[test]
    fn explicit_default_only_target_is_rejected_in_current_mode() {
        assert!(matches!(
            resolve(&["docker".to_string()], false, ScopeMode::Current),
            Err(AppError::UnsupportedCurrentModeTarget(_))
        ));
    }
}
