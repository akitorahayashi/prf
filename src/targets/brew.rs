use crate::cleanup::{Discovery, Rule, ScopeSupport, Target, TargetId};

const RULES: &[Rule] =
    &[Rule::HomePaths { paths: &["Library/Caches/Homebrew", "Library/Logs/Homebrew"] }];

pub(super) static TARGET: Target = Target::new(
    TargetId::new("brew"),
    "Homebrew",
    ScopeSupport::DefaultOnly,
    Discovery::Rules(RULES),
);
