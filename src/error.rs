use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;

use thiserror::Error;

/// Application-wide error type for the prf CLI.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Unknown target '{0}'")]
    InvalidTarget(String),

    #[error("Target index out of range: {0}")]
    TargetIndexOutOfRange(String),

    #[error("Target not supported with --current: {0}")]
    UnsupportedCurrentModeTarget(String),

    #[error("Invalid target registry: {0}")]
    InvalidTargetRegistry(String),

    #[error("Discovery failed: {0}")]
    Discovery(String),

    #[error("Cleanup failed: {0}")]
    Cleanup(String),

    #[error("Cannot {operation} '{}': {source}", path.display())]
    PathOperation {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("Cannot start {label} with '{program}': {source}")]
    ProcessStart {
        label: &'static str,
        program: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("{label} failed with status {status} while running '{program}'")]
    ProcessExit { label: &'static str, program: &'static str, status: ExitStatus },

    #[error("Cleanup incomplete: {retained} retained, {failed} failed")]
    IncompleteCleanup { retained: usize, failed: usize },

    #[error("Footprint estimation failed: {0}")]
    Footprint(#[from] crate::footprint::Error),

    #[error(
        "Cannot determine the default scan root because HOME is not set. Pass a path argument or use --current."
    )]
    HomeUnset,

    #[error("Operation cancelled by user")]
    Cancelled,
}
