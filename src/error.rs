use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Application-wide error type for the prf CLI (purify).
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

    #[error("No targets to scan: {0}")]
    NoTargetsToScan(String),

    #[error("Home directory is unavailable; pass a PATH or use --current")]
    HomeDirectoryUnavailable,

    #[error("Current directory is unavailable: {0}")]
    CurrentDirectoryUnavailable(io::Error),

    #[error("Invalid scan root '{}': {reason}", path.display())]
    InvalidRoot { path: PathBuf, reason: String },

    #[error("Cannot traverse '{}': {reason}", path.display())]
    Traversal { path: PathBuf, reason: String },

    #[error("Cannot measure '{}': {reason}", path.display())]
    Measurement { path: PathBuf, reason: String },

    #[error("{category} is unavailable: {reason}")]
    CategoryUnavailable { category: String, reason: String },

    #[error("Scan is incomplete: {0}")]
    IncompleteScan(String),

    #[error("External command failed: {0}")]
    ExternalCommand(String),

    #[error("Candidate revalidation failed for '{}': {reason}", path.display())]
    Revalidation { path: PathBuf, reason: String },

    #[error("Cannot remove '{}': {reason}", path.display())]
    Removal { path: PathBuf, reason: String },

    #[error("Cleanup completed with failures: {0}")]
    CleanupFailed(String),

    #[error("Operation cancelled by user")]
    Cancelled,
}
