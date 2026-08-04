pub mod action;
pub mod apply;
pub mod candidate;
pub mod discovery;
pub mod environment;
pub mod estimate;
pub mod plan;
mod removal_path;
pub mod report;
pub mod scope;
pub mod target;

pub use action::{Action, EntryKind};
pub use apply::{ActionOutcome, ApplyReport, PathStatus, ProcessStatus, apply_plan};
pub use candidate::Candidate;
pub use discovery::{Discovery, Inspection, InspectionInputs, Listing, Rule};
pub use estimate::{ActionEstimate, EstimateSummary};
pub use plan::RemovalCatalog;
pub use report::ScanReport;
#[cfg(test)]
pub use scope::Scope;
pub use scope::ScopeMode;
pub use target::{ScopeSupport, Target, TargetId};
