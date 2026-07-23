use crate::cleanup::{Discovery, Rule, ScopeSupport, Target, TargetId};

const RULES: &[Rule] =
    &[Rule::DirectoryNames { names: &["target"], parent_marker: Some("Cargo.toml") }];

pub(super) static TARGET: Target =
    Target::new(TargetId::new("rust"), "Rust", ScopeSupport::AllModes, Discovery::Rules(RULES));
