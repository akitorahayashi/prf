use std::collections::HashSet;
use std::path::{Path, PathBuf};

use dirs_next as dirs;
use walkdir::WalkDir;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::target::{CleanupTarget, MAX_SCAN_DEPTH, ScanScope};

enum LocalMatch {
    DerivedData(PathBuf),
    SwiftPackage(PathBuf),
}

pub struct XcodeTarget {
    current: bool,
}

impl XcodeTarget {
    pub fn new(current: bool) -> Self {
        Self { current }
    }

    fn global_safe_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let lib = home.join("Library");
            paths.push(lib.join("Developer/Xcode/DerivedData"));
            paths.push(lib.join("Caches/com.apple.dt.Xcode"));
            paths.push(lib.join("Developer/Xcode/DocumentationCache"));
            paths.push(lib.join("Developer/Xcode/DocumentationIndex"));
            paths.push(lib.join("Developer/Xcode/UserData/Previews"));
            paths.push(lib.join("Caches/org.swift.swiftpm"));
            paths.push(lib.join("org.swift.swiftpm"));
            paths.push(lib.join("Developer/CoreSimulator/Caches"));
        }
        paths
    }

    fn add_path(&self, path: &Path, items: &mut Vec<CleanupItem>) {
        let item = if path.is_file() {
            CleanupItem::file(Category::Xcode, path.to_path_buf(), 0)
        } else {
            CleanupItem::directory(Category::Xcode, path.to_path_buf(), 0)
        };
        items.push(item);
    }

    fn collect_swiftpm_artifacts(&self, parent: &Path, items: &mut Vec<CleanupItem>) {
        const ARTIFACTS: &[&str] = &[".build", ".swiftpm"];
        for artifact in ARTIFACTS {
            let artifact_path = parent.join(artifact);
            if artifact_path.exists() {
                self.add_path(&artifact_path, items);
            }
        }
    }

    fn scan_global_caches(&self) -> Vec<CleanupItem> {
        let mut items = Vec::new();
        for path in Self::global_safe_paths() {
            if path.exists() {
                self.add_path(&path, &mut items);
            }
        }
        items
    }

    /// Single local traversal shared by discovery and listing. Yields each DerivedData
    /// directory and each SwiftPM package root (deduplicated), so both consumers see the
    /// same matches without walking the tree twice.
    fn for_each_local_match<F: FnMut(LocalMatch)>(&self, scope: &ScanScope, mut visit: F) {
        let mut processed_packages: HashSet<PathBuf> = HashSet::new();

        for root in scope.roots() {
            if !root.exists() {
                continue;
            }

            let mut walker = WalkDir::new(root).max_depth(MAX_SCAN_DEPTH).into_iter();
            while let Some(entry) = walker.next() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(err) => {
                        if scope.verbose() {
                            eprintln!("Skipping {:?}: {}", err.path(), err);
                        }
                        continue;
                    }
                };

                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy();

                if entry.file_type().is_dir() && file_name == "DerivedData" {
                    visit(LocalMatch::DerivedData(path.to_path_buf()));
                    walker.skip_current_dir();
                    continue;
                }

                if entry.file_type().is_file()
                    && file_name == "Package.swift"
                    && let Some(parent) = path.parent()
                    && processed_packages.insert(parent.to_path_buf())
                {
                    visit(LocalMatch::SwiftPackage(parent.to_path_buf()));
                }
            }
        }
    }

    fn scan_local_projects(&self, scope: &ScanScope) -> Vec<CleanupItem> {
        let mut items = Vec::new();
        self.for_each_local_match(scope, |matched| match matched {
            LocalMatch::DerivedData(path) => self.add_path(&path, &mut items),
            LocalMatch::SwiftPackage(parent) => self.collect_swiftpm_artifacts(&parent, &mut items),
        });
        items
    }

    fn list_global_targets(&self) -> Vec<String> {
        let mut targets = Vec::new();
        for path in Self::global_safe_paths() {
            if path.exists() {
                targets.push(format!("{} (exists)", path.display()));
            }
        }
        targets
    }

    fn list_local_targets(&self, scope: &ScanScope) -> Vec<String> {
        let mut targets = Vec::new();
        let mut derived_data = 0usize;
        let mut swiftpm_projects = 0usize;

        self.for_each_local_match(scope, |matched| match matched {
            LocalMatch::DerivedData(_) => derived_data += 1,
            LocalMatch::SwiftPackage(_) => swiftpm_projects += 1,
        });

        if derived_data > 0 {
            targets.push(format!(
                "DerivedData ({} location{} found)",
                derived_data,
                if derived_data == 1 { "" } else { "s" }
            ));
        }

        if swiftpm_projects > 0 {
            targets.push(format!(
                "SwiftPM Projects (.build, .swiftpm) ({} location{} found)",
                swiftpm_projects,
                if swiftpm_projects == 1 { "" } else { "s" }
            ));
        }

        targets
    }
}

impl CleanupTarget for XcodeTarget {
    fn category(&self) -> Category {
        Category::Xcode
    }

    fn discover(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        let mut items = self.scan_local_projects(scope);
        if !self.current {
            let mut global_items = self.scan_global_caches();
            items.append(&mut global_items);
        }
        Ok(items)
    }

    fn list(&self, scope: &ScanScope) -> Result<Vec<String>, AppError> {
        let mut targets = self.list_local_targets(scope);
        if !self.current {
            let mut global = self.list_global_targets();
            targets.append(&mut global);
        }
        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use serial_test::serial;
    use std::env;

    use super::*;
    use crate::targets::item::CleanupAction;

    fn item_paths(items: &[CleanupItem]) -> Vec<String> {
        items
            .iter()
            .filter_map(|item| match &item.action {
                CleanupAction::Path { path, .. } => Some(path.to_string_lossy().into_owned()),
                CleanupAction::DockerPrune => None,
            })
            .collect()
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
        let items = target.discover(&scope).expect("scan succeeds");

        assert!(
            item_paths(&items).iter().any(|path| path.ends_with("DerivedData")),
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
        let items = target.discover(&scope).expect("scan succeeds");

        let paths = item_paths(&items);
        assert!(
            paths.iter().any(|path| path.contains("AppWithPackage/.build")),
            ".build directory should be reported when Package.swift exists"
        );
        assert!(
            paths.iter().any(|path| path.contains("AppWithPackage/.swiftpm")),
            ".swiftpm directory should be reported when Package.swift exists"
        );
        assert!(
            !paths.iter().any(|path| path.contains("AppWithPackage/Package.resolved")),
            "Package.resolved should not be reported even if Package.swift exists"
        );
        assert!(
            !paths.iter().any(|path| path.contains("AppWithoutPackage/.build")),
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
        let items = target.discover(&scope).expect("scan succeeds");
        assert!(
            item_paths(&items)
                .iter()
                .any(|path| path.contains("Library/Developer/Xcode/DerivedData")),
            "global caches should be detected when not in current-only mode"
        );

        let current_scope = ScanScope::new(Vec::new(), true, false);
        let current_target = XcodeTarget::new(true);
        let current_items = current_target.discover(&current_scope).expect("scan succeeds");
        assert!(current_items.is_empty(), "--current should skip global caches");
    }
}
