use std::fmt;

use crate::error::AppError;

use super::discovery::{Discovery, Inspection, InspectionInputs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetId(&'static str);

impl TargetId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for TargetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeSupport {
    AllModes,
    DefaultOnly,
}

impl ScopeSupport {
    pub const fn supports_current(self) -> bool {
        matches!(self, Self::AllModes)
    }
}

pub struct Target {
    id: TargetId,
    display_name: &'static str,
    scope_support: ScopeSupport,
    discovery: Discovery,
}

impl Target {
    pub const fn new(
        id: TargetId,
        display_name: &'static str,
        scope_support: ScopeSupport,
        discovery: Discovery,
    ) -> Self {
        Self { id, display_name, scope_support, discovery }
    }

    pub const fn id(&self) -> TargetId {
        self.id
    }

    pub const fn display_name(&self) -> &'static str {
        self.display_name
    }

    pub const fn scope_support(&self) -> ScopeSupport {
        self.scope_support
    }

    pub fn inspect(&self, inputs: &InspectionInputs) -> Result<Inspection, AppError> {
        if inputs.scope().is_current() && !self.scope_support.supports_current() {
            return Err(AppError::UnsupportedCurrentModeTarget(self.id.to_string()));
        }
        self.discovery.inspect(self.id, inputs)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unexpected_inspection(_: TargetId, _: &InspectionInputs) -> Result<Inspection, AppError> {
        panic!("unsupported target inspection must not run")
    }

    #[test]
    fn current_scope_rejects_default_only_target_before_inspection() {
        let target = Target::new(
            TargetId::new("global"),
            "Global",
            ScopeSupport::DefaultOnly,
            Discovery::Inspector(unexpected_inspection),
        );
        let scope = crate::cleanup::Scope::resolve(true, Some("/home".into()), "/working".into())
            .expect("current scope resolves");
        let inputs = InspectionInputs::for_test(scope);

        assert!(matches!(
            target.inspect(&inputs),
            Err(AppError::UnsupportedCurrentModeTarget(name)) if name == "global"
        ));
    }
}
