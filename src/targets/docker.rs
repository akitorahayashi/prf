use std::io;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use byte_unit::Byte;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::target::{CleanupTarget, ScanScope};

static DOCKER_AVAILABLE: OnceLock<bool> = OnceLock::new();

fn probe_docker_available() -> bool {
    Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn docker_available() -> bool {
    *DOCKER_AVAILABLE.get_or_init(probe_docker_available)
}

fn parse_reclaimable_size(size_token: &str) -> Option<Byte> {
    if let Ok(size) = Byte::parse_str(size_token, true) {
        return Some(size);
    }

    let split_index = size_token
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_digit() || *ch == '.'))
        .map(|(index, _)| index)?;

    let (num, unit) = size_token.split_at(split_index);
    if num.is_empty() || unit.trim().is_empty() {
        return None;
    }

    let normalized = format!("{} {}", num, unit.trim());
    Byte::parse_str(&normalized, true).ok()
}

pub fn run_cleanup(verbose: bool) -> Result<(), AppError> {
    if !docker_available() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Docker CLI not available").into());
    }

    let args = ["system", "prune", "-a", "-f", "--volumes"];
    if verbose {
        println!("$ docker {}", args.join(" "));
    }

    let status = Command::new("docker").args(args).status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "docker {} failed with status {}",
            args.join(" "),
            status
        ))
        .into());
    }

    Ok(())
}

pub struct DockerTarget;

impl DockerTarget {
    pub fn new() -> Self {
        Self
    }

    fn available(&self) -> bool {
        docker_available()
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

    fn discover(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        if !self.available() {
            if scope.verbose() {
                println!("Docker CLI not available, skipping Docker scan.");
            }
            return Ok(Vec::new());
        }

        let output =
            Command::new("docker").args(["system", "df", "--format", "{{json .}}"]).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = if stderr.trim().is_empty() {
                format!("'docker system df' exited with status {}", output.status)
            } else {
                format!("'docker system df' failed: {}", stderr.trim())
            };
            if scope.verbose() {
                eprintln!("{message}");
            }
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total = 0u64;

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parsed = serde_json::from_str::<serde_json::Value>(line).ok();
            let Some(json) = parsed else {
                continue;
            };

            let Some(reclaimable_str) = json.get("Reclaimable").and_then(|value| value.as_str())
            else {
                continue;
            };

            let Some(size_token) = reclaimable_str.split_whitespace().next() else {
                continue;
            };

            if let Some(size) = parse_reclaimable_size(size_token) {
                total = total.saturating_add(size.as_u64());
            }
        }

        if total == 0 { Ok(Vec::new()) } else { Ok(vec![CleanupItem::docker_prune(total)]) }
    }

    fn list(&self, _scope: &ScanScope) -> Result<Vec<String>, AppError> {
        if !self.available() {
            return Ok(Vec::new());
        }

        Ok(vec![
            "Unused images".to_string(),
            "Stopped containers".to_string(),
            "Unused volumes".to_string(),
            "Unused networks".to_string(),
            "Build cache".to_string(),
        ])
    }
}
