use std::ffi::OsString;
use std::path::PathBuf;

use super::estimate::ActionEstimate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    RemovePath { path: PathBuf, kind: EntryKind },
    RunProcess { label: String, program: String, args: Vec<OsString>, estimate: ActionEstimate },
}
