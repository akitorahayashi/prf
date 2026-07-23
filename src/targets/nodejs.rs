use crate::error::AppError;

use super::category::Category;
use super::name_matcher::NameMatcherTarget;
use super::target::{CleanupTarget, DiscoveryOutcome, ScanScope};

const NODEJS_TARGETS: &[&str] = &["node_modules", ".next", ".nuxt", ".svelte-kit"];

pub struct NodejsTarget(NameMatcherTarget);

impl NodejsTarget {
    pub fn new() -> Self {
        Self(NameMatcherTarget::new(Category::Nodejs, NODEJS_TARGETS))
    }
}

impl Default for NodejsTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupTarget for NodejsTarget {
    fn category(&self) -> Category {
        self.0.category()
    }

    fn discover(&self, scope: &ScanScope) -> Result<DiscoveryOutcome, AppError> {
        self.0.discover(scope)
    }
}
