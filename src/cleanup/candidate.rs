use std::path::PathBuf;

use crate::footprint::Estimate;

use super::action::{Action, EntryKind};
use super::target::TargetId;

#[derive(Debug, Clone)]
pub struct Candidate {
    target: TargetId,
    action: Action,
}

impl Candidate {
    pub fn directory(target: TargetId, path: PathBuf) -> Self {
        Self { target, action: Action::RemovePath { path, kind: EntryKind::Directory } }
    }

    pub fn file(target: TargetId, path: PathBuf) -> Self {
        Self { target, action: Action::RemovePath { path, kind: EntryKind::File } }
    }

    pub fn symlink(target: TargetId, path: PathBuf) -> Self {
        Self { target, action: Action::RemovePath { path, kind: EntryKind::Symlink } }
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
            action: Action::RunProcess {
                label,
                program,
                args,
                estimate: Estimate::from_bytes(reported_bytes),
            },
        }
    }

    pub const fn target(&self) -> TargetId {
        self.target
    }

    pub const fn action(&self) -> &Action {
        &self.action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: TargetId = TargetId::new("test");

    #[test]
    fn filesystem_candidates_are_path_actions() {
        let candidate = Candidate::directory(TARGET, PathBuf::from("target"));

        assert!(matches!(candidate.action(), Action::RemovePath { .. }));
    }

    #[test]
    fn process_candidates_own_their_reported_estimate() {
        let candidate = Candidate::process(TARGET, "process", "program", &["arg"], 42);

        assert!(matches!(
            candidate.action(),
            Action::RunProcess { estimate, .. } if *estimate == Estimate::from_bytes(42)
        ));
    }
}
