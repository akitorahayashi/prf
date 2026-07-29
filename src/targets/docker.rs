use std::io::ErrorKind;
use std::process::Command;

use byte_unit::Byte;

use crate::cleanup::{
    Candidate, Discovery, Inspection, Listing, Scope, ScopeSupport, Target, TargetId,
};
use crate::error::AppError;

const PRUNE_ARGS: &[&str] = &["system", "prune", "-a", "-f", "--volumes"];
const PRUNE_LABEL: &str =
    "Docker prune: unused images, containers, networks, build cache, and volumes (-a --volumes)";
const LISTINGS: &[&str] =
    &["Unused images", "Stopped containers", "Unused volumes", "Unused networks", "Build cache"];

pub(super) static TARGET: Target = Target::new(
    TargetId::new("docker"),
    "Docker",
    ScopeSupport::DefaultOnly,
    Discovery::Inspector(inspect),
);

fn inspect(target: TargetId, _scope: &Scope) -> Result<Inspection, AppError> {
    let availability = match Command::new("docker").arg("info").output() {
        Ok(output) => output,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(Inspection::diagnostic("Docker CLI is unavailable"));
        }
        Err(error) => return Err(AppError::Io(error)),
    };

    if !availability.status.success() {
        let stderr = String::from_utf8_lossy(&availability.stderr);
        let detail = stderr.trim();
        return Ok(Inspection::diagnostic(if detail.is_empty() {
            format!("Docker daemon is unavailable: {}", availability.status)
        } else {
            format!("Docker daemon is unavailable: {detail}")
        }));
    }

    let output =
        Command::new("docker").args(["system", "df", "--format", "{{json .}}"]).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(AppError::Discovery(if detail.is_empty() {
            format!("docker system df failed with status {}", output.status)
        } else {
            format!("docker system df failed: {detail}")
        }));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        AppError::Discovery(format!("docker system df returned invalid UTF-8: {error}"))
    })?;
    let reclaimable = parse_reclaimable_total(&stdout)?;
    let candidates = if reclaimable == 0 {
        Vec::new()
    } else {
        vec![Candidate::process(target, PRUNE_LABEL, "docker", PRUNE_ARGS, reclaimable)]
    };

    Ok(Inspection {
        candidates,
        listings: LISTINGS.iter().map(|label| Listing::Detail((*label).to_string())).collect(),
        diagnostics: Vec::new(),
    })
}

fn parse_reclaimable_total(stdout: &str) -> Result<u64, AppError> {
    let mut total = 0u64;
    for (index, line) in stdout.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let json: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            AppError::Discovery(format!(
                "docker system df line {} is not valid JSON: {error}",
                index + 1
            ))
        })?;
        let reclaimable =
            json.get("Reclaimable").and_then(serde_json::Value::as_str).ok_or_else(|| {
                AppError::Discovery(format!(
                    "docker system df line {} has no string Reclaimable field",
                    index + 1
                ))
            })?;
        let token = reclaimable.split_whitespace().next().ok_or_else(|| {
            AppError::Discovery(format!(
                "docker system df line {} has an empty Reclaimable field",
                index + 1
            ))
        })?;
        total = total
            .checked_add(parse_reclaimable_size(token)?.as_u64())
            .ok_or_else(|| AppError::Discovery("Docker reclaimable total overflow".to_string()))?;
    }
    Ok(total)
}

fn parse_reclaimable_size(token: &str) -> Result<Byte, AppError> {
    if let Ok(size) = Byte::parse_str(token, true) {
        return Ok(size);
    }

    let split_index = token
        .char_indices()
        .find(|(_, character)| !(character.is_ascii_digit() || *character == '.'))
        .map(|(index, _)| index)
        .ok_or_else(|| AppError::Discovery(format!("invalid Docker size '{token}'")))?;
    let (number, unit) = token.split_at(split_index);
    if number.is_empty() || unit.trim().is_empty() {
        return Err(AppError::Discovery(format!("invalid Docker size '{token}'")));
    }

    Byte::parse_str(format!("{} {}", number, unit.trim()), true)
        .map_err(|error| AppError::Discovery(format!("invalid Docker size '{token}': {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reclaimable_sizes_from_json_lines() {
        let total = parse_reclaimable_total(
            "{\"Type\":\"Images\",\"Reclaimable\":\"1.5GB (50%)\"}\n\
             {\"Type\":\"Containers\",\"Reclaimable\":\"500MB\"}\n",
        )
        .expect("Docker output parses");

        assert_eq!(total, 2_000_000_000);
    }

    #[test]
    fn malformed_json_is_an_explicit_failure() {
        assert!(matches!(parse_reclaimable_total("not-json"), Err(AppError::Discovery(_))));
    }

    #[test]
    fn missing_reclaimable_field_is_an_explicit_failure() {
        assert!(matches!(
            parse_reclaimable_total("{\"Type\":\"Images\"}"),
            Err(AppError::Discovery(_))
        ));
    }

    #[test]
    fn reclaimable_total_overflow_is_an_explicit_failure() {
        assert!(matches!(
            parse_reclaimable_total(
                "{\"Reclaimable\":\"18446744073709551615B\"}\n\
                 {\"Reclaimable\":\"1B\"}"
            ),
            Err(AppError::Discovery(message)) if message.contains("overflow")
        ));
    }
}
