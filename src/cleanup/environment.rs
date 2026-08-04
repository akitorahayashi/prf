use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct EnvironmentPaths {
    home: Option<PathBuf>,
    working_directory: PathBuf,
    temporary_directory: PathBuf,
    xdg_cache_home: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    mise_cache_dir: Option<PathBuf>,
    bun_install_cache_dir: Option<PathBuf>,
}

impl EnvironmentPaths {
    pub fn capture() -> Result<Self, AppError> {
        Ok(Self {
            home: std::env::var_os("HOME").map(PathBuf::from),
            working_directory: std::env::current_dir()?,
            temporary_directory: std::env::temp_dir(),
            xdg_cache_home: xdg_path(std::env::var_os("XDG_CACHE_HOME")),
            xdg_config_home: xdg_path(std::env::var_os("XDG_CONFIG_HOME")),
            mise_cache_dir: std::env::var_os("MISE_CACHE_DIR").map(PathBuf::from),
            bun_install_cache_dir: std::env::var_os("BUN_INSTALL_CACHE_DIR").map(PathBuf::from),
        })
    }

    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    pub fn temporary_directory(&self) -> &Path {
        &self.temporary_directory
    }

    pub fn xdg_cache_home(&self) -> Option<&Path> {
        self.xdg_cache_home.as_deref()
    }

    pub fn xdg_config_home(&self) -> Option<&Path> {
        self.xdg_config_home.as_deref()
    }

    pub fn mise_cache_dir(&self) -> Option<&Path> {
        self.mise_cache_dir.as_deref()
    }

    pub fn bun_install_cache_dir(&self) -> Option<&Path> {
        self.bun_install_cache_dir.as_deref()
    }

    #[cfg(test)]
    pub fn for_test(working_directory: PathBuf) -> Self {
        Self {
            home: None,
            temporary_directory: working_directory.join("temporary"),
            working_directory,
            xdg_cache_home: None,
            xdg_config_home: None,
            mise_cache_dir: None,
            bun_install_cache_dir: None,
        }
    }
}

fn xdg_path(value: Option<OsString>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_paths_ignore_empty_and_relative_values() {
        let absolute = std::env::temp_dir().join("prf-xdg");

        assert_eq!(xdg_path(None), None);
        assert_eq!(xdg_path(Some(OsString::new())), None);
        assert_eq!(xdg_path(Some(OsString::from("relative/cache"))), None);
        assert_eq!(xdg_path(Some(absolute.clone().into_os_string())), Some(absolute));
    }
}
