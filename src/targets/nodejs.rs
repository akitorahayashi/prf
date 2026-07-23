use crate::cleanup::{Discovery, Rule, ScopeSupport, Target, TargetId};

const RULES: &[Rule] = &[Rule::DirectoryNames {
    names: &["node_modules", ".next", ".nuxt", ".svelte-kit"],
    parent_marker: None,
}];

pub(super) static TARGET: Target = Target::new(
    TargetId::new("nodejs"),
    "Node.js",
    ScopeSupport::AllModes,
    Discovery::Rules(RULES),
);
