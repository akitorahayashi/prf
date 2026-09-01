use std::path::{Path, PathBuf};

use crate::cleanup::discovery::inspect_external_path;
use crate::cleanup::{Discovery, Inspection, InspectionInputs, ScopeSupport, Target, TargetId};
use crate::error::AppError;

pub(super) static TARGET: Target = Target::new(
    TargetId::new("mise"),
    "mise",
    ScopeSupport::DefaultOnly,
    Discovery::Inspector(inspect),
);

fn inspect(target: TargetId, inputs: &InspectionInputs) -> Result<Inspection, AppError> {
    let environment = inputs.environment();
    let path = resolve_cache_path(
        environment.mise_cache_dir(),
        environment.xdg_cache_home(),
        environment.home(),
    )?;
    inspect_external_path(target, inputs, path)
}

fn resolve_cache_path(
    configured: Option<&Path>,
    xdg_cache_home: Option<&Path>,
    home: Option<&Path>,
) -> Result<PathBuf, AppError> {
    if let Some(path) = configured {
        return Ok(path.to_path_buf());
    }
    if let Some(path) = xdg_cache_home {
        return Ok(path.join("mise"));
    }
    home.map(|home| home.join("Library/Caches/mise")).ok_or_else(|| {
        AppError::Discovery("HOME is unavailable while resolving the mise cache".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_uses_documented_precedence() {
        let home = Path::new("/Users/test");

        assert_eq!(
            resolve_cache_path(Some(Path::new("/cache/mise")), Some(Path::new("/xdg")), Some(home))
                .expect("configured path resolves"),
            PathBuf::from("/cache/mise")
        );
        assert_eq!(
            resolve_cache_path(None, Some(Path::new("/xdg")), Some(home))
                .expect("XDG path resolves"),
            PathBuf::from("/xdg/mise")
        );
        assert_eq!(
            resolve_cache_path(None, None, Some(home)).expect("macOS default resolves"),
            PathBuf::from("/Users/test/Library/Caches/mise")
        );
    }
}
