pub mod action;
pub mod apply;
pub mod candidate;
pub mod discovery;
pub mod plan;
pub mod report;
pub mod scope;
pub mod target;

pub use action::{Action, EntryKind};
pub use apply::{ActionOutcome, ApplyReport, PathStatus, ProcessStatus, apply_plan};
pub use candidate::Candidate;
pub use discovery::{Diagnostic, Discovery, Inspection, Listing, Rule};
pub use plan::{RemovalCatalog, RemovalPlan};
pub use report::{ScanReport, TargetReport};
pub use scope::Scope;
pub use target::{ScopeSupport, Target, TargetId};
