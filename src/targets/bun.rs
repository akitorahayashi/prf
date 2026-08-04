use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::cleanup::discovery::inspect_path;
use crate::cleanup::{Discovery, Inspection, InspectionInputs, ScopeSupport, Target, TargetId};
use crate::error::AppError;

pub(super) static TARGET: Target = Target::new(
    TargetId::new("bun"),
    "Bun",
    ScopeSupport::DefaultOnly,
    Discovery::Inspector(inspect),
);

fn inspect(target: TargetId, inputs: &InspectionInputs) -> Result<Inspection, AppError> {
    let environment = inputs.environment();
    let path = resolve_cache_path(
        environment.bun_install_cache_dir(),
        environment.xdg_config_home(),
        environment.home(),
    )?;
    inputs.validate_external_cache_path(&path)?;
    Ok(inspect_path(target, path))
}

fn resolve_cache_path(
    configured: Option<&Path>,
    xdg_config_home: Option<&Path>,
    home: Option<&Path>,
) -> Result<PathBuf, AppError> {
    let home = home.ok_or_else(|| {
        AppError::Discovery("HOME is unavailable while resolving the Bun cache".to_string())
    })?;
    if let Some(path) = configured {
        return expand_home(path, home);
    }

    let config_path = xdg_config_home
        .map(|directory| directory.join(".bunfig.toml"))
        .unwrap_or_else(|| home.join(".bunfig.toml"));
    if let Some(path) = configured_cache_path(&config_path)? {
        return expand_home(&path, home);
    }

    Ok(home.join(".bun/install/cache"))
}

fn configured_cache_path(config_path: &Path) -> Result<Option<PathBuf>, AppError> {
    let contents = match fs::read_to_string(config_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(AppError::Discovery(format!(
                "unable to read Bun config {}: {error}",
                config_path.display()
            )));
        }
    };
    let config = toml::from_str::<toml::Value>(&contents).map_err(|error| {
        AppError::Discovery(format!("invalid Bun config {}: {error}", config_path.display()))
    })?;
    let Some(install) = config.get("install") else {
        return Ok(None);
    };
    let install = install.as_table().ok_or_else(|| {
        AppError::Discovery(format!(
            "Bun config {} has a non-table install value",
            config_path.display()
        ))
    })?;
    let Some(cache) = install.get("cache") else {
        return Ok(None);
    };
    let cache = cache.as_table().ok_or_else(|| {
        AppError::Discovery(format!(
            "Bun config {} has a non-table install.cache value",
            config_path.display()
        ))
    })?;
    let Some(directory) = cache.get("dir") else {
        return Ok(None);
    };
    let directory = directory.as_str().ok_or_else(|| {
        AppError::Discovery(format!(
            "Bun config {} has a non-string install.cache.dir",
            config_path.display()
        ))
    })?;
    Ok(Some(PathBuf::from(directory)))
}

fn expand_home(path: &Path, home: &Path) -> Result<PathBuf, AppError> {
    if path == Path::new("~") {
        return Ok(home.to_path_buf());
    }
    if let Ok(relative) = path.strip_prefix("~/") {
        return Ok(home.join(relative));
    }
    if path.starts_with("~") {
        return Err(AppError::Discovery(format!(
            "unsupported home expansion in Bun cache path {}",
            path.display()
        )));
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;

    #[test]
    fn environment_override_precedes_global_config() {
        let temp = TempDir::new().expect("temporary directory exists");
        temp.child(".bunfig.toml")
            .write_str("[install.cache]\ndir = '/config/cache'\n")
            .expect("config exists");

        assert_eq!(
            resolve_cache_path(Some(Path::new("/environment/cache")), None, Some(temp.path()))
                .expect("cache resolves"),
            PathBuf::from("/environment/cache")
        );
    }

    #[test]
    fn global_config_expands_home_and_invalid_config_fails() {
        let temp = TempDir::new().expect("temporary directory exists");
        let config = temp.child(".bunfig.toml");
        config.write_str("[install.cache]\ndir = '~/.cache/bun'\n").expect("config exists");

        assert_eq!(
            resolve_cache_path(None, None, Some(temp.path())).expect("cache resolves"),
            temp.path().join(".cache/bun")
        );

        config.write_str("install = 'invalid'\n").expect("invalid type replaces fixture");
        assert!(matches!(
            resolve_cache_path(None, None, Some(temp.path())),
            Err(AppError::Discovery(message)) if message.contains("non-table install value")
        ));

        config.write_str("[install.cache\n").expect("invalid config replaces fixture");
        assert!(matches!(
            resolve_cache_path(None, None, Some(temp.path())),
            Err(AppError::Discovery(message)) if message.contains("invalid Bun config")
        ));
    }

    #[test]
    fn absent_configuration_uses_the_documented_default() {
        assert_eq!(
            resolve_cache_path(None, None, Some(Path::new("/Users/test"))).expect("cache resolves"),
            PathBuf::from("/Users/test/.bun/install/cache")
        );
    }
}
