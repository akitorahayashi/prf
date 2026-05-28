use std::path::PathBuf;

use dirs_next as dirs;

use crate::error::AppError;

use super::category::Category;
use super::item::CleanupItem;
use super::name_matcher::NameMatcherTarget;
use super::target::{CleanupTarget, ScanScope};

const NODEJS_TARGETS: &[&str] = &[
    "node_modules",
    ".pnpm-store",
    ".next",
    ".nuxt",
    ".svelte-kit",
    "playwright-report",
    "test-results",
];

pub struct NodejsTarget {
    matcher: NameMatcherTarget,
    current: bool,
}

impl NodejsTarget {
    pub fn new(current: bool) -> Self {
        Self { matcher: NameMatcherTarget::new(Category::Nodejs, NODEJS_TARGETS), current }
    }

    fn global_safe_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(cache) = dirs::cache_dir() {
            paths.push(cache.join("ms-playwright"));
            paths.push(cache.join("pnpm"));
        }

        if let Some(data_local) = dirs::data_local_dir() {
            paths.push(data_local.join("pnpm/store"));
        }

        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Library/pnpm/store"));
        }

        paths
    }
}

impl CleanupTarget for NodejsTarget {
    fn category(&self) -> Category {
        Category::Nodejs
    }

    fn discover(&self, scope: &ScanScope) -> Result<Vec<CleanupItem>, AppError> {
        let mut items = self.matcher.discover(scope)?;

        if !self.current {
            for path in Self::global_safe_paths() {
                if path.exists() {
                    items.push(CleanupItem::directory(Category::Nodejs, path, 0));
                }
            }
        }

        Ok(items)
    }

    fn list(&self, scope: &ScanScope) -> Result<Vec<String>, AppError> {
        let mut targets = self.matcher.list(scope)?;

        if !self.current {
            for path in Self::global_safe_paths() {
                if path.exists() {
                    targets.push(format!("{} (exists)", path.display()));
                }
            }
        }

        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;

    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use serial_test::serial;

    use super::*;

    struct EnvGuard {
        vars: Vec<(String, Option<String>)>,
    }

    impl EnvGuard {
        fn set(vars: &[(&str, &Path)]) -> Self {
            let mut saved = Vec::new();
            for (key, val) in vars {
                saved.push((key.to_string(), env::var(key).ok()));
                unsafe {
                    env::set_var(key, val);
                }
            }
            Self { vars: saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, val) in &self.vars {
                if let Some(v) = val {
                    unsafe {
                        env::set_var(key, v);
                    }
                } else {
                    unsafe {
                        env::remove_var(key);
                    }
                }
            }
        }
    }

    #[test]
    fn discover_detects_local_nodejs_targets() {
        let temp = TempDir::new().expect("temp directory is created");
        let project_root = temp.child("workspace");
        project_root.create_dir_all().expect("workspace exists");

        // Create local targets
        let node_modules = project_root.child("node_modules");
        node_modules.create_dir_all().expect("node_modules exists");

        let pnpm_store = project_root.child(".pnpm-store");
        pnpm_store.create_dir_all().expect(".pnpm-store exists");

        let playwright_report = project_root.child("playwright-report");
        playwright_report.create_dir_all().expect("playwright-report exists");

        let test_results = project_root.child("test-results");
        test_results.create_dir_all().expect("test-results exists");

        let target = NodejsTarget::new(true);
        let scope = ScanScope::new(vec![project_root.path().to_path_buf()], true, true);
        let items = target.discover(&scope).expect("scan succeeds");

        let paths: Vec<_> =
            items.iter().map(|i| i.path.file_name().unwrap().to_str().unwrap()).collect();
        assert!(paths.contains(&"node_modules"));
        assert!(paths.contains(&".pnpm-store"));
        assert!(paths.contains(&"playwright-report"));
        assert!(paths.contains(&"test-results"));
    }

    #[test]
    #[serial]
    fn discover_global_caches_respects_current_flag() {
        let temp_home = TempDir::new().expect("temp home is created");

        // On macOS, cache_dir is ~/Library/Caches, on Linux it is ~/.cache
        // dirs_next depends on env vars.
        let cache_dir = if cfg!(target_os = "macos") {
            temp_home.child("Library/Caches")
        } else {
            temp_home.child(".cache")
        };

        let playwright = cache_dir.child("ms-playwright");
        playwright.create_dir_all().expect("playwright cache exists");

        // Mock environment variables for dirs-next
        let _guard = if cfg!(target_os = "macos") {
            EnvGuard::set(&[("HOME", temp_home.path())])
        } else {
            EnvGuard::set(&[("HOME", temp_home.path()), ("XDG_CACHE_HOME", cache_dir.path())])
        };

        let scope = ScanScope::new(Vec::new(), false, false);
        let target = NodejsTarget::new(false);
        let items = target.discover(&scope).expect("scan succeeds");

        assert!(
            items.iter().any(|item| item.path.to_string_lossy().contains("ms-playwright")),
            "global caches should be detected when not in current-only mode"
        );

        let current_target = NodejsTarget::new(true);
        let current_items = current_target.discover(&scope).expect("scan succeeds");
        assert!(
            !current_items.iter().any(|item| item.path.to_string_lossy().contains("ms-playwright")),
            "--current should skip global caches"
        );
    }
}
