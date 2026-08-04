use std::ffi::OsString;
use std::path::PathBuf;

use super::action::{Action, EntryKind};
use super::estimate::ActionEstimate;
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
        label: impl Into<String>,
        program: impl Into<String>,
        args: Vec<OsString>,
        estimate: ActionEstimate,
    ) -> Self {
        Self {
            target,
            action: Action::RunProcess {
                label: label.into(),
                program: program.into(),
                args,
                estimate,
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
    fn process_candidates_own_dynamic_arguments_and_their_estimate_basis() {
        let candidate = Candidate::process(
            TARGET,
            "process",
            "program",
            vec![OsString::from("arg")],
            ActionEstimate::Unestimated,
        );

        assert!(matches!(
            candidate.action(),
            Action::RunProcess { args, estimate, .. }
                if args == &[OsString::from("arg")] && *estimate == ActionEstimate::Unestimated
        ));
    }
}
