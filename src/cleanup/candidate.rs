use std::path::PathBuf;

use crate::footprint::{Basis, Estimate};

use super::action::{Action, EntryKind};
use super::target::TargetId;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub target: TargetId,
    pub action: Action,
    basis: Basis,
}

impl Candidate {
    pub fn directory(target: TargetId, path: PathBuf) -> Self {
        Self {
            target,
            action: Action::RemovePath { path, kind: EntryKind::Directory },
            basis: Basis::Allocated,
        }
    }

    pub fn file(target: TargetId, path: PathBuf) -> Self {
        Self {
            target,
            action: Action::RemovePath { path, kind: EntryKind::File },
            basis: Basis::Allocated,
        }
    }

    pub fn process(
        target: TargetId,
        label: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        reported_bytes: u64,
    ) -> Self {
        Self {
            target,
            action: Action::RunProcess { label, program, args },
            basis: Basis::Reported(Estimate::from_bytes(reported_bytes)),
        }
    }

    pub const fn basis(&self) -> Basis {
        self.basis
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: TargetId = TargetId::new("test");

    #[test]
    fn filesystem_candidates_have_an_allocated_basis() {
        let candidate = Candidate::directory(TARGET, PathBuf::from("target"));

        assert_eq!(candidate.basis(), Basis::Allocated);
    }

    #[test]
    fn process_candidates_have_a_reported_basis() {
        let candidate = Candidate::process(TARGET, "process", "program", &["arg"], 42);

        assert_eq!(candidate.basis(), Basis::Reported(Estimate::from_bytes(42)));
    }
}
