use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Command;

use crate::cleanup::{
    ActionEstimate, Candidate, Discovery, Inspection, InspectionInputs, Listing, ScopeSupport,
    Target, TargetId,
};
use crate::error::AppError;

const PRUNE_LABEL: &str = "pnpm store prune: unreferenced packages";

pub(super) static TARGET: Target = Target::new(
    TargetId::new("pnpm"),
    "pnpm",
    ScopeSupport::DefaultOnly,
    Discovery::Inspector(inspect),
);

fn inspect(target: TargetId, inputs: &InspectionInputs) -> Result<Inspection, AppError> {
    let output = match Command::new("pnpm").args(["store", "path", "--silent"]).output() {
        Ok(output) => output,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(Inspection::diagnostic("pnpm CLI is unavailable"));
        }
        Err(error) => return Err(AppError::Io(error)),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(AppError::Discovery(if detail.is_empty() {
            format!("pnpm store path failed with status {}", output.status)
        } else {
            format!("pnpm store path failed: {detail}")
        }));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        AppError::Discovery(format!("pnpm store path returned invalid UTF-8: {error}"))
    })?;
    let path = parse_store_path(&stdout)?;
    inputs.validate_external_cache_path(&path)?;
    match path.symlink_metadata() {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Inspection::default()),
        Err(source) => {
            return Err(AppError::path_operation("inspect pnpm store path", path, source));
        }
    }
    let path = fs::canonicalize(&path).map_err(|source| {
        AppError::path_operation("resolve pnpm store path", path.clone(), source)
    })?;
    let metadata = fs::metadata(&path).map_err(|source| {
        AppError::path_operation("inspect resolved pnpm store path", path.clone(), source)
    })?;
    if !metadata.is_dir() {
        return Err(AppError::Discovery(format!(
            "pnpm store path is not a directory: {}",
            path.display()
        )));
    }
    inputs.validate_external_cache_path(&path)?;

    let mut store_argument = OsString::from("--store-dir=");
    store_argument.push(path.as_os_str());
    Ok(Inspection {
        candidates: vec![Candidate::process(
            target,
            PRUNE_LABEL,
            "pnpm",
            vec![store_argument, OsString::from("store"), OsString::from("prune")],
            ActionEstimate::Unestimated,
        )],
        listings: vec![
            Listing::Path(path),
            Listing::Detail("Unreferenced packages via pnpm store prune".to_string()),
        ],
        diagnostics: Vec::new(),
    })
}

fn parse_store_path(stdout: &str) -> Result<PathBuf, AppError> {
    let mut lines = stdout.lines().filter(|line| !line.trim().is_empty());
    let path = lines
        .next()
        .ok_or_else(|| AppError::Discovery("pnpm store path returned an empty path".to_string()))?;
    if lines.next().is_some() {
        return Err(AppError::Discovery(
            "pnpm store path returned multiple non-empty lines".to_string(),
        ));
    }
    let path = PathBuf::from(path.trim());
    if !path.is_absolute() {
        return Err(AppError::Discovery(format!(
            "pnpm store path is not absolute: {}",
            path.display()
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_path_requires_one_absolute_line() {
        assert_eq!(
            parse_store_path("/Users/test/Library/pnpm/store/v11\n").expect("store path parses"),
            PathBuf::from("/Users/test/Library/pnpm/store/v11")
        );
        assert!(parse_store_path("").is_err());
        assert!(parse_store_path("relative/store\n").is_err());
        assert!(parse_store_path("/first\n/second\n").is_err());
    }
}
