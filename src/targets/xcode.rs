use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;

use super::category::Category;
use super::item::{CleanupItem, PathAuthority};
use super::target::{CleanupTarget, DiscoveryOutcome, ScanScope};
use super::traversal::{VisitControl, visit_roots};

pub struct XcodeTarget {
    current: bool,
}

impl XcodeTarget {
    pub fn new(current: bool) -> Self {
        Self { current }
    }

    fn global_safe_paths() -> Result<Vec<PathBuf>, AppError> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(AppError::HomeDirectoryUnavailable)?;
        let lib = home.join("Library");
        Ok(vec![
            lib.join("Developer/Xcode/DerivedData"),
            lib.join("Caches/com.apple.dt.Xcode"),
            lib.join("Developer/Xcode/DocumentationCache"),
            lib.join("Developer/Xcode/DocumentationIndex"),
            lib.join("Developer/Xcode/UserData/Previews"),
            lib.join("Caches/org.swift.swiftpm"),
            lib.join("org.swift.swiftpm"),
            lib.join("Developer/CoreSimulator/Caches"),
        ])
    }

    fn add_path(
        &self,
        path: &Path,
        authority: PathAuthority,
        items: &mut Vec<CleanupItem>,
    ) -> Result<(), AppError> {
        items.push(CleanupItem::from_path(Category::Xcode, path.to_path_buf(), authority)?);
        Ok(())
    }

    fn collect_swiftpm_artifacts(
        &self,
        parent: &Path,
        authority: &PathAuthority,
        items: &mut Vec<CleanupItem>,
    ) -> Result<(), AppError> {
        const ARTIFACTS: &[&str] = &[".build", ".swiftpm"];
        for artifact in ARTIFACTS {
            let artifact_path = parent.join(artifact);
            match fs::symlink_metadata(&artifact_path) {
                Ok(_) => self.add_path(&artifact_path, authority.clone(), items)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AppError::Traversal {
                        path: artifact_path,
                        reason: error.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    fn scan_global_caches(&self) -> Result<Vec<CleanupItem>, AppError> {
        let mut items = Vec::new();
        for path in Self::global_safe_paths()? {
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    let authority = CleanupItem::user_authority(&path)?;
                    self.add_path(&path, authority, &mut items)?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(AppError::Traversal { path, reason: error.to_string() });
                }
            }
        }
        Ok(items)
    }

    fn scan_local_projects(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        let mut items = Vec::new();
        let mut processed_packages: HashSet<PathBuf> = HashSet::new();

        visit_roots(scope, |root, entry| {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy();
            let authority = CleanupItem::local_authority(root)?;

            if entry.file_type().is_dir() && file_name == "DerivedData" {
                self.add_path(path, authority, &mut items)?;
                return Ok(VisitControl::SkipDirectory);
            }

            if entry.file_type().is_file()
                && file_name == "Package.swift"
                && let Some(parent) = path.parent()
                && processed_packages.insert(parent.to_path_buf())
            {
                self.collect_swiftpm_artifacts(parent, &authority, &mut items)?;
            }
            Ok(VisitControl::Continue)
        })?;

        Ok(items)
    }
}

impl CleanupTarget for XcodeTarget {
    fn category(&self) -> Category {
        Category::Xcode
    }

    fn discover(&self, scope: &ScanScope) -> Result<DiscoveryOutcome, AppError> {
        let mut items = self.scan_local_projects(scope)?;
        if !self.current {
            let mut global_items = self.scan_global_caches()?;
            items.append(&mut global_items);
        }
        Ok(DiscoveryOutcome::Complete(items))
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use serial_test::serial;
    use std::env;

    use super::*;

    fn complete_items(outcome: DiscoveryOutcome) -> Vec<CleanupItem> {
        match outcome {
            DiscoveryOutcome::Complete(items) => items,
            DiscoveryOutcome::Unavailable(reason) => {
                panic!("unexpected unavailable target: {reason}")
            }
        }
    }

    struct HomeGuard {
        original_home: Option<String>,
    }

    impl HomeGuard {
        fn set(temp_home: &Path) -> Self {
            let original_home = env::var("HOME").ok();
            unsafe {
                env::set_var("HOME", temp_home);
            }
            Self { original_home }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            if let Some(home) = &self.original_home {
                unsafe {
                    env::set_var("HOME", home);
                }
            } else {
                unsafe {
                    env::remove_var("HOME");
                }
            }
        }
    }

    #[test]
    fn discover_detects_local_derived_data() {
        let temp = TempDir::new().expect("temp directory is created");
        let project_root = temp.child("workspace");
        project_root.create_dir_all().expect("workspace exists");
        let derived = project_root.child("DerivedData/cache");
        derived.create_dir_all().expect("derived data exists");
        derived.child("foo.txt").write_str("cache").expect("cache file exists");

        let target = XcodeTarget::new(false);
        let scope = ScanScope::new(vec![project_root.path().to_path_buf()], false, true);
        let items = complete_items(target.discover(&scope).expect("scan succeeds"));

        assert!(
            items.iter().any(|item| item.path().is_some_and(|path| path.ends_with("DerivedData"))),
            "expected DerivedData directory to be reported"
        );
    }

    #[test]
    fn discover_detects_swiftpm_artifacts_only_with_package_swift() {
        let temp = TempDir::new().expect("temp directory is created");
        let roots = temp.child("workspace");
        roots.create_dir_all().expect("workspace exists");

        let pkg = roots.child("AppWithPackage");
        pkg.create_dir_all().expect("package workspace exists");
        pkg.child("Package.swift").write_str("// swift package").expect("package file exists");
        pkg.child(".build/output.o").write_str("bin").expect("build artifact exists");
        pkg.child(".swiftpm/config").write_str("cfg").expect("swiftpm artifact exists");
        pkg.child("Package.resolved").write_str("deps").expect("resolved file exists");

        let no_pkg = roots.child("AppWithoutPackage");
        no_pkg.create_dir_all().expect("non-package workspace exists");
        no_pkg.child(".build/output.o").write_str("bin").expect("build artifact exists");

        let target = XcodeTarget::new(false);
        let scope = ScanScope::new(vec![roots.path().to_path_buf()], false, true);
        let items = complete_items(target.discover(&scope).expect("scan succeeds"));

        assert!(
            items.iter().any(|item| item
                .path()
                .is_some_and(|path| path.to_string_lossy().contains("AppWithPackage/.build"))),
            ".build directory should be reported when Package.swift exists"
        );
        assert!(
            items.iter().any(|item| item
                .path()
                .is_some_and(|path| path.to_string_lossy().contains("AppWithPackage/.swiftpm"))),
            ".swiftpm directory should be reported when Package.swift exists"
        );
        assert!(
            !items.iter().any(|item| item.path().is_some_and(|path| path
                .to_string_lossy()
                .contains("AppWithPackage/Package.resolved"))),
            "Package.resolved should not be reported even if Package.swift exists"
        );
        assert!(
            !items.iter().any(|item| item
                .path()
                .is_some_and(|path| path.to_string_lossy().contains("AppWithoutPackage/.build"))),
            "projects without Package.swift should be ignored"
        );
    }

    #[test]
    #[serial]
    fn discover_global_caches_respects_current_flag() {
        let temp_home = TempDir::new().expect("temp home is created");
        let derived = temp_home.child("Library/Developer/Xcode/DerivedData/project");
        derived.create_dir_all().expect("derived data exists");
        derived.child("foo.txt").write_str("cache").expect("cache file exists");

        let _home_guard = HomeGuard::set(temp_home.path());

        let scope = ScanScope::new(Vec::new(), false, false);
        let target = XcodeTarget::new(false);
        let items = complete_items(target.discover(&scope).expect("scan succeeds"));
        assert!(
            items.iter().any(|item| item.path().is_some_and(|path| path
                .to_string_lossy()
                .contains("Library/Developer/Xcode/DerivedData"))),
            "global caches should be detected when not in current-only mode"
        );

        let current_scope = ScanScope::new(Vec::new(), true, false);
        let current_target = XcodeTarget::new(true);
        let current_items =
            complete_items(current_target.discover(&current_scope).expect("scan succeeds"));
        assert!(current_items.is_empty(), "--current should skip global caches");
    }
}
