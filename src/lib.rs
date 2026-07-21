//! Library entry point for the prf CLI.

pub mod app;
pub mod cli;
pub mod error;
pub mod fs;
pub mod output;
pub mod report;
pub mod targets;

pub use error::AppError;
