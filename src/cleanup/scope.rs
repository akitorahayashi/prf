use std::path::{Path, PathBuf};

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeMode {
    Default,
    Current,
}

#[derive(Debug, Clone)]
pub enum Scope {
    Default { root: PathBuf, home: PathBuf },
    Current { root: PathBuf },
}

impl Scope {
    pub fn resolve(
        current: bool,
        home: Option<PathBuf>,
        working_directory: PathBuf,
    ) -> Result<Self, AppError> {
        if current {
            return Ok(Self::Current { root: working_directory });
        }

        let home = home.ok_or(AppError::HomeUnset)?;
        let root = home.join("Desktop");
        Ok(Self::Default { root, home })
    }

    pub fn root(&self) -> &Path {
        match self {
            Self::Default { root, .. } | Self::Current { root } => root,
        }
    }

    pub const fn mode(&self) -> ScopeMode {
        match self {
            Self::Default { .. } => ScopeMode::Default,
            Self::Current { .. } => ScopeMode::Current,
        }
    }

    pub fn home(&self) -> Option<&Path> {
        match self {
            Self::Default { home, .. } => Some(home),
            Self::Current { .. } => None,
        }
    }
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
            current: bool,
            home: Option<PathBuf>,
            expected_root: Result<PathBuf, &'static str>,
            expected_mode: ScopeMode,
            expected_home: Option<PathBuf>,
        }

        let cases = [
            Case {
                current: false,
                home: Some(path("/home/user")),
                expected_root: Ok(path("/home/user/Desktop")),
                expected_mode: ScopeMode::Default,
                expected_home: Some(path("/home/user")),
            },
            Case {
                current: true,
                home: Some(path("/home/user")),
                expected_root: Ok(path("/working")),
                expected_mode: ScopeMode::Current,
                expected_home: None,
            },
            Case {
                current: true,
                home: None,
                expected_root: Ok(path("/working")),
                expected_mode: ScopeMode::Current,
                expected_home: None,
            },
            Case {
                current: false,
                home: None,
                expected_root: Err("home"),
                expected_mode: ScopeMode::Default,
                expected_home: None,
            },
        ];

        for case in cases {
            let result = Scope::resolve(case.current, case.home, path("/working"));
            match case.expected_root {
                Ok(expected) => {
                    let scope = result.expect("scope resolves");
                    assert_eq!(scope.root(), expected);
                    assert_eq!(scope.mode(), case.expected_mode);
                    assert_eq!(scope.home(), case.expected_home.as_deref());
                }
                Err("home") => assert!(matches!(result, Err(AppError::HomeUnset))),
                Err(other) => panic!("unknown expected error: {other}"),
            }
        }
    }
}
