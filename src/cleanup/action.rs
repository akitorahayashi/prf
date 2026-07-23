use std::path::PathBuf;

use crate::footprint::Estimate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    RemovePath {
        path: PathBuf,
        kind: EntryKind,
    },
    RunProcess {
        label: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        estimate: Estimate,
    },
}
