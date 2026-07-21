use std::io;

use thiserror::Error;

/// Application-wide error type for the prf CLI.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Unknown category '{0}'")]
    InvalidCategory(String),

    #[error("Category index out of range: {0}")]
    CategoryIndexOutOfRange(String),

    #[error("Category not supported with --current: {0}")]
    UnsupportedCurrentModeCategory(String),

    #[error(
        "Cannot determine the default scan root because HOME is not set. Pass a path argument or use --current."
    )]
    HomeUnset,

    #[error("Operation cancelled by user")]
    Cancelled,
}
