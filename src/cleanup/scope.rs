use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeMode {
    Default,
    Current,
}

#[derive(Debug, Clone)]
pub enum Scope {
    Default { roots: Vec<PathBuf>, home: Option<PathBuf> },
    Current { root: PathBuf },
}

impl Scope {
    pub fn from_environment(explicit: &[PathBuf], current: bool) -> Result<Self, AppError> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let working_directory = std::env::current_dir()?;
        Self::resolve(explicit, current, home, working_directory)
    }

    pub fn resolve(
        explicit: &[PathBuf],
        current: bool,
        home: Option<PathBuf>,
        working_directory: PathBuf,
    ) -> Result<Self, AppError> {
        if current {
            if !explicit.is_empty() {
                return Err(AppError::InvalidScope(
                    "current-directory scope cannot include explicit roots".to_string(),
                ));
            }
            return Ok(Self::Current { root: working_directory });
        }

        let roots = if explicit.is_empty() {
            vec![home.as_ref().ok_or(AppError::HomeUnset)?.join("Desktop")]
        } else {
            deduplicate(explicit)
        };
        Ok(Self::Default { roots, home })
    }

    pub fn roots(&self) -> &[PathBuf] {
        match self {
            Self::Default { roots, .. } => roots,
            Self::Current { root } => std::slice::from_ref(root),
        }
    }

    pub const fn mode(&self) -> ScopeMode {
        match self {
            Self::Default { .. } => ScopeMode::Default,
            Self::Current { .. } => ScopeMode::Current,
        }
    }

    pub const fn is_current(&self) -> bool {
        matches!(self, Self::Current { .. })
    }

    pub fn home(&self) -> Option<&Path> {
        match self {
            Self::Default { home, .. } => home.as_deref(),
            Self::Current { .. } => None,
        }
    }
}

fn deduplicate(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths.iter().filter(|path| seen.insert((*path).clone())).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn resolution_table_covers_supported_scope_inputs() {
        struct Case {
            explicit: Vec<PathBuf>,
            current: bool,
            home: Option<PathBuf>,
            expected_roots: Result<Vec<PathBuf>, &'static str>,
            expected_mode: ScopeMode,
            expected_home: Option<PathBuf>,
        }

        let cases = [
            Case {
                explicit: Vec::new(),
                current: false,
                home: Some(path("/home/user")),
                expected_roots: Ok(vec![path("/home/user/Desktop")]),
                expected_mode: ScopeMode::Default,
                expected_home: Some(path("/home/user")),
            },
            Case {
                explicit: vec![path("/first"), path("/second")],
                current: false,
                home: Some(path("/home/user")),
                expected_roots: Ok(vec![path("/first"), path("/second")]),
                expected_mode: ScopeMode::Default,
                expected_home: Some(path("/home/user")),
            },
            Case {
                explicit: Vec::new(),
                current: true,
                home: Some(path("/home/user")),
                expected_roots: Ok(vec![path("/working")]),
                expected_mode: ScopeMode::Current,
                expected_home: None,
            },
            Case {
                explicit: Vec::new(),
                current: false,
                home: None,
                expected_roots: Err("home"),
                expected_mode: ScopeMode::Default,
                expected_home: None,
            },
            Case {
                explicit: vec![path("/same"), path("/same"), path("/same/child")],
                current: false,
                home: None,
                expected_roots: Ok(vec![path("/same"), path("/same/child")]),
                expected_mode: ScopeMode::Default,
                expected_home: None,
            },
        ];

        for case in cases {
            let result = Scope::resolve(&case.explicit, case.current, case.home, path("/working"));
            match case.expected_roots {
                Ok(expected) => {
                    let scope = result.expect("scope resolves");
                    assert_eq!(scope.roots(), expected);
                    assert_eq!(scope.mode(), case.expected_mode);
                    assert_eq!(scope.home(), case.expected_home.as_deref());
                }
                Err("home") => assert!(matches!(result, Err(AppError::HomeUnset))),
                Err(other) => panic!("unknown expected error: {other}"),
            }
        }
    }

    #[test]
    fn current_scope_rejects_explicit_roots() {
        assert!(matches!(
            Scope::resolve(&[path("/explicit")], true, Some(path("/home")), path("/working")),
            Err(AppError::InvalidScope(_))
        ));
    }
}
