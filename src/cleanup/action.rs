use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    RemovePath { path: PathBuf, kind: EntryKind },
    RunProcess { label: &'static str, program: &'static str, args: &'static [&'static str] },
}

impl Action {
    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::RemovePath { path, .. } => Some(path),
            Self::RunProcess { .. } => None,
        }
    }
}
