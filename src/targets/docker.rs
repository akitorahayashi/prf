use std::io;
use std::process::{Command, Stdio};

use byte_unit::Byte;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::target::{CleanupTarget, DiscoveryOutcome, ScanScope};

fn docker_available() -> bool {
    Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn parse_reclaimable_size(size_token: &str) -> Result<Byte, AppError> {
    if let Ok(size) = Byte::parse_str(size_token, true) {
        return Ok(size);
    }

    let split_index = size_token
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_digit() || *ch == '.'))
        .map(|(index, _)| index)
        .ok_or_else(|| {
            AppError::ExternalCommand(format!(
                "Docker returned an invalid reclaimable size '{size_token}'"
            ))
        })?;

    let (number, unit) = size_token.split_at(split_index);
    let normalized = format!("{} {}", number, unit.trim());
    Byte::parse_str(&normalized, true).map_err(|error| {
        AppError::ExternalCommand(format!(
            "Docker returned an invalid reclaimable size '{size_token}': {error}"
        ))
    })
}

pub fn run_cleanup(verbose: bool) -> Result<(), AppError> {
    if !docker_available() {
        return Err(AppError::CategoryUnavailable {
            category: Category::Docker.display_name().to_string(),
            reason: "Docker CLI is unavailable or the daemon is not running".to_string(),
        });
    }

    let args = ["system", "prune", "--all", "--force", "--volumes"];
    if verbose {
        eprintln!("$ docker {}", args.join(" "));
    }

    let status = Command::new("docker").args(args).status()?;
    if !status.success() {
        return Err(AppError::ExternalCommand(format!(
            "docker {} exited with status {status}",
            args.join(" ")
        )));
    }

    Ok(())
}

pub struct DockerTarget;

impl DockerTarget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DockerTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupTarget for DockerTarget {
    fn category(&self) -> Category {
        Category::Docker
    }

    fn discover(&self, _scope: &ScanScope) -> Result<DiscoveryOutcome, AppError> {
        if !docker_available() {
            return Ok(DiscoveryOutcome::Unavailable(
                "Docker CLI is unavailable or the daemon is not running".to_string(),
            ));
        }

        let output =
            Command::new("docker").args(["system", "df", "--format", "{{json .}}"]).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let reason = if stderr.trim().is_empty() {
                format!("docker system df exited with status {}", output.status)
            } else {
                format!("docker system df failed: {}", stderr.trim())
            };
            return Err(AppError::ExternalCommand(reason));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total = 0u64;

        for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
            let json = serde_json::from_str::<serde_json::Value>(line).map_err(|error| {
                AppError::ExternalCommand(format!(
                    "Docker returned malformed disk-usage JSON: {error}"
                ))
            })?;
            let reclaimable =
                json.get("Reclaimable").and_then(|value| value.as_str()).ok_or_else(|| {
                    AppError::ExternalCommand(
                        "Docker disk-usage output has no Reclaimable value".to_string(),
                    )
                })?;
            let size_token = reclaimable.split_whitespace().next().ok_or_else(|| {
                AppError::ExternalCommand("Docker returned an empty reclaimable size".to_string())
            })?;
            total = total.saturating_add(parse_reclaimable_size(size_token)?.as_u64());
        }

        let items = if total == 0 { Vec::new() } else { vec![CleanupItem::docker_prune(total)] };
        Ok(DiscoveryOutcome::Complete(items))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reclaimable_size_parser_accepts_compact_docker_units() {
        assert_eq!(parse_reclaimable_size("1.5GB").expect("size parses").as_u64(), 1_500_000_000);
    }

    #[test]
    fn reclaimable_size_parser_rejects_invalid_values() {
        assert!(parse_reclaimable_size("unknown").is_err());
    }
}
