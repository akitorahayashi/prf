use std::io;

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

    #[error(
        "Cannot determine the default scan root because HOME is not set. Pass a path argument or use --current."
    )]
    HomeUnset,

    #[error("Operation cancelled by user")]
    Cancelled,
}
