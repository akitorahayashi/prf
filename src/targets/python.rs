use crate::cleanup::{Discovery, Rule, ScopeSupport, Target, TargetId};

const RULES: &[Rule] = &[Rule::DirectoryNames {
    names: &["__pycache__", ".pytest_cache", ".ruff_cache", ".mypy_cache", ".venv"],
    parent_marker: None,
}];

pub(super) static TARGET: Target =
    Target::new(TargetId::new("python"), "Python", ScopeSupport::AllModes, Discovery::Rules(RULES));
