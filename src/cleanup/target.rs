use std::fmt;

use crate::error::AppError;

use super::discovery::{Discovery, Inspection};
use super::scope::Scope;

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

    pub fn inspect(&self, scope: &Scope) -> Result<Inspection, AppError> {
        self.discovery.inspect(self.id, scope)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(formatter)
    }
}
