use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;

pub fn resolve_roots(explicit: &[PathBuf]) -> Result<Vec<PathBuf>, AppError> {
    let requested = if explicit.is_empty() {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(AppError::HomeDirectoryUnavailable)?;
        vec![home.join("Desktop")]
    } else {
        explicit.to_vec()
    };

    normalize_roots(&requested)
}

pub fn resolve_roots_with_current(
    explicit: &[PathBuf],
    current: bool,
) -> Result<Vec<PathBuf>, AppError> {
    debug_assert!(!current || explicit.is_empty());

    if current {
        let current = std::env::current_dir().map_err(AppError::CurrentDirectoryUnavailable)?;
        normalize_roots(&[current])
    } else {
        resolve_roots(explicit)
    }
}

fn normalize_roots(requested: &[PathBuf]) -> Result<Vec<PathBuf>, AppError> {
    let mut roots =
        requested.iter().map(|path| validate_root(path)).collect::<Result<Vec<_>, _>>()?;
    roots.sort_by(|left, right| {
        left.components().count().cmp(&right.components().count()).then_with(|| left.cmp(right))
    });

    let mut normalized = Vec::new();
    for root in roots {
        if normalized.iter().any(|parent: &PathBuf| root.starts_with(parent)) {
            continue;
        }
        normalized.push(root);
    }

    Ok(normalized)
}

fn validate_root(path: &Path) -> Result<PathBuf, AppError> {
    let invalid = |reason: String| AppError::InvalidRoot { path: path.to_path_buf(), reason };
    let canonical = fs::canonicalize(path).map_err(|err| invalid(err.to_string()))?;
    let metadata = fs::metadata(&canonical).map_err(|err| invalid(err.to_string()))?;
    if !metadata.is_dir() {
        return Err(invalid("path is not a directory".to_string()));
    }
    fs::read_dir(&canonical).map_err(|err| invalid(err.to_string()))?;
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use serial_test::serial;

    struct EnvGuard {
        home: Option<String>,
        cwd: PathBuf,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                home: std::env::var("HOME").ok(),
                cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(home) = &self.home {
                unsafe {
                    std::env::set_var("HOME", home);
                }
            } else {
                unsafe {
                    std::env::remove_var("HOME");
                }
            }

            let _ = std::env::set_current_dir(&self.cwd);
        }
    }

    #[test]
    fn resolve_roots_returns_explicit_roots_when_non_empty() {
        let temp = TempDir::new().expect("temp directory is created");
        let first = temp.child("a");
        let second = temp.child("b");
        first.create_dir_all().expect("first root exists");
        second.create_dir_all().expect("second root exists");
        let explicit = vec![first.path().to_path_buf(), second.path().to_path_buf()];
        let expected = explicit
            .iter()
            .map(|path| std::fs::canonicalize(path).expect("test root canonicalizes"))
            .collect::<Vec<_>>();

        assert_eq!(resolve_roots(&explicit).expect("roots resolve"), expected);
    }

    #[test]
    #[serial]
    fn resolve_roots_uses_home_desktop_when_explicit_empty() {
        let _guard = EnvGuard::new();
        let temp_home = TempDir::new().expect("temp home is created");
        temp_home.child("Desktop").create_dir_all().expect("Desktop exists");

        unsafe {
            std::env::set_var("HOME", temp_home.path());
        }

        let roots = resolve_roots(&[]).expect("default root resolves");
        let expected =
            std::fs::canonicalize(temp_home.path().join("Desktop")).expect("Desktop canonicalizes");
        assert_eq!(roots, vec![expected]);
    }

    #[test]
    #[serial]
    fn resolve_roots_with_current_prefers_current_dir() {
        let _guard = EnvGuard::new();
        let temp = TempDir::new().expect("temp directory is created");
        std::env::set_current_dir(temp.path()).expect("cwd is set");

        let roots = resolve_roots_with_current(&[], true).expect("current root resolves");
        let expected = std::env::current_dir().expect("cwd resolves");
        assert_eq!(roots, vec![expected]);
    }

    #[test]
    #[serial]
    fn resolve_roots_fails_when_home_is_unset() {
        let _guard = EnvGuard::new();

        unsafe {
            std::env::remove_var("HOME");
        }

        assert!(matches!(resolve_roots(&[]), Err(AppError::HomeDirectoryUnavailable)));
    }

    #[test]
    fn resolve_roots_collapses_overlapping_roots() {
        let temp = TempDir::new().expect("temp directory is created");
        let nested = temp.child("workspace/project");
        nested.create_dir_all().expect("nested root exists");

        let roots = resolve_roots(&[
            nested.path().to_path_buf(),
            temp.path().to_path_buf(),
            temp.path().to_path_buf(),
        ])
        .expect("roots resolve");

        let expected = std::fs::canonicalize(temp.path()).expect("root canonicalizes");
        assert_eq!(roots, vec![expected]);
    }

    #[test]
    fn resolve_roots_rejects_files() {
        let temp = TempDir::new().expect("temp directory is created");
        let file = temp.child("not-a-directory");
        file.write_str("content").expect("file exists");

        assert!(matches!(
            resolve_roots(&[file.path().to_path_buf()]),
            Err(AppError::InvalidRoot { .. })
        ));
    }
}
