use std::collections::{BTreeMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::AppError;

use super::candidate::Candidate;
use super::environment::EnvironmentPaths;
use super::removal_path::validate_removal_path;
use super::scope::Scope;
use super::target::TargetId;

const MAX_SCAN_DEPTH: usize = 10;

pub type InspectFn = fn(TargetId, &InspectionInputs) -> Result<Inspection, AppError>;

#[derive(Debug, Clone)]
pub struct InspectionInputs {
    scope: Scope,
    environment: EnvironmentPaths,
}

impl InspectionInputs {
    pub fn from_environment(current: bool) -> Result<Self, AppError> {
        let environment = EnvironmentPaths::capture()?;
        let scope = Scope::resolve(
            current,
            environment.home().map(Path::to_path_buf),
            environment.working_directory().to_path_buf(),
        )?;
        Ok(Self { scope, environment })
    }

    #[cfg(test)]
    pub const fn new(scope: Scope, environment: EnvironmentPaths) -> Self {
        Self { scope, environment }
    }

    pub const fn scope(&self) -> &Scope {
        &self.scope
    }

    pub const fn environment(&self) -> &EnvironmentPaths {
        &self.environment
    }

    pub fn protected_paths(&self) -> Result<Vec<PathBuf>, AppError> {
        let mut lexical_paths = self.scope.roots().to_vec();
        lexical_paths.push(self.environment.working_directory().to_path_buf());
        lexical_paths.push(self.environment.temporary_directory().to_path_buf());
        if let Some(home) = self.environment.home() {
            lexical_paths.push(home.to_path_buf());
        }

        let mut paths = Vec::new();
        for path in lexical_paths {
            if !paths.contains(&path) {
                paths.push(path.clone());
            }
            match path.canonicalize() {
                Ok(canonical) if !paths.contains(&canonical) => paths.push(canonical),
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(AppError::PathOperation {
                        operation: "resolve protected path",
                        path,
                        source,
                    });
                }
            }
        }
        Ok(paths)
    }

    pub fn validate_external_cache_path(&self, path: &Path) -> Result<(), AppError> {
        let protected_paths = self.protected_paths()?;
        validate_removal_path(path, &protected_paths).map_err(AppError::Discovery)
    }

    #[cfg(test)]
    pub fn for_test(scope: Scope) -> Self {
        let working_directory = scope.roots()[0].clone();
        Self::new(scope, EnvironmentPaths::for_test(working_directory))
    }
}

#[derive(Clone, Copy)]
pub enum Discovery {
    Rules(&'static [Rule]),
    Inspector(InspectFn),
}

impl Discovery {
    pub fn inspect(
        self,
        target: TargetId,
        inputs: &InspectionInputs,
    ) -> Result<Inspection, AppError> {
        match self {
            Self::Rules(rules) => inspect_rules(target, inputs.scope(), rules),
            Self::Inspector(inspect) => inspect(target, inputs),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Rule {
    DirectoryNames {
        names: &'static [&'static str],
        parent_marker: Option<&'static str>,
    },
    MarkerChildren {
        marker: &'static str,
        children: &'static [&'static str],
        listing: &'static str,
    },
    HomePaths {
        paths: &'static [&'static str],
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listing {
    Count { label: String, count: usize },
    Path(PathBuf),
    Detail(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Inspection {
    pub candidates: Vec<Candidate>,
    pub listings: Vec<Listing>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Inspection {
    pub fn diagnostic(message: impl Into<String>) -> Self {
        Self {
            candidates: Vec::new(),
            listings: Vec::new(),
            diagnostics: vec![Diagnostic { message: message.into() }],
        }
    }
}

pub(crate) fn inspect_path(target: TargetId, path: PathBuf) -> Inspection {
    let mut inspection = Inspection::default();
    let mut candidate_paths = HashSet::new();
    match path.symlink_metadata() {
        Ok(_) => {
            inspection.listings.push(Listing::Path(path.clone()));
            add_existing_path(target, path, &mut inspection, &mut candidate_paths);
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => inspection.diagnostics.push(Diagnostic {
            message: format!("Unable to inspect cleanup entry {}: {error}", path.display()),
        }),
    }
    inspection
}

fn inspect_rules(
    target: TargetId,
    scope: &Scope,
    rules: &'static [Rule],
) -> Result<Inspection, AppError> {
    let mut inspection = Inspection::default();
    let mut listing_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut candidate_paths = HashSet::new();
    let mut processed_markers: HashSet<(usize, PathBuf)> = HashSet::new();

    let has_root_rules = rules.iter().any(|rule| !matches!(rule, Rule::HomePaths { .. }));
    if has_root_rules {
        inspect_roots(
            target,
            scope,
            rules,
            &mut inspection,
            &mut listing_counts,
            &mut candidate_paths,
            &mut processed_markers,
        );
    }

    if !scope.is_current() {
        inspect_home_paths(target, scope, rules, &mut inspection, &mut candidate_paths);
    }

    inspection.listings.splice(
        0..0,
        listing_counts.into_iter().map(|(label, count)| Listing::Count { label, count }),
    );
    Ok(inspection)
}

fn inspect_roots(
    target: TargetId,
    scope: &Scope,
    rules: &[Rule],
    inspection: &mut Inspection,
    listing_counts: &mut BTreeMap<String, usize>,
    candidate_paths: &mut HashSet<PathBuf>,
    processed_markers: &mut HashSet<(usize, PathBuf)>,
) {
    for root in scope.roots() {
        if !root.exists() {
            inspection.diagnostics.push(Diagnostic {
                message: format!("Scan root does not exist: {}", root.display()),
            });
            continue;
        }

        let mut walker = WalkDir::new(root).max_depth(MAX_SCAN_DEPTH).into_iter();
        while let Some(result) = walker.next() {
            let entry = match result {
                Ok(entry) => entry,
                Err(error) => {
                    inspection.diagnostics.push(Diagnostic {
                        message: format!("Unable to inspect {:?}: {error}", error.path()),
                    });
                    continue;
                }
            };

            let mut skip_current = false;
            for (index, rule) in rules.iter().enumerate() {
                match rule {
                    Rule::DirectoryNames { names, parent_marker } => {
                        if !entry.file_type().is_dir() && !entry.file_type().is_symlink() {
                            continue;
                        }
                        let name = entry.file_name().to_string_lossy();
                        if !names.contains(&name.as_ref()) {
                            continue;
                        }
                        if parent_marker.is_some_and(|marker| {
                            !entry
                                .path()
                                .parent()
                                .is_some_and(|parent| parent.join(marker).is_file())
                        }) {
                            continue;
                        }

                        let path = entry.path().to_path_buf();
                        if candidate_paths.insert(path.clone()) {
                            add_classified_path(target, path, inspection);
                        }
                        *listing_counts.entry(name.into_owned()).or_default() += 1;
                        skip_current = true;
                    }
                    Rule::MarkerChildren { marker, children, listing } => {
                        if !entry.file_type().is_file() || entry.file_name() != *marker {
                            continue;
                        }
                        let Some(parent) = entry.path().parent() else {
                            continue;
                        };
                        let parent = parent.to_path_buf();
                        if !processed_markers.insert((index, parent.clone())) {
                            continue;
                        }

                        *listing_counts.entry((*listing).to_string()).or_default() += 1;
                        for child in *children {
                            let path = parent.join(child);
                            add_existing_path(target, path, inspection, candidate_paths);
                        }
                    }
                    Rule::HomePaths { .. } => {}
                }
            }

            if skip_current {
                walker.skip_current_dir();
            }
        }
    }
}

fn inspect_home_paths(
    target: TargetId,
    scope: &Scope,
    rules: &[Rule],
    inspection: &mut Inspection,
    candidate_paths: &mut HashSet<PathBuf>,
) {
    let home_paths = rules.iter().filter_map(|rule| match rule {
        Rule::HomePaths { paths } => Some(*paths),
        _ => None,
    });

    let mut saw_home_rule = false;
    let Some(home) = scope.home() else {
        if home_paths.count() > 0 {
            inspection.diagnostics.push(Diagnostic {
                message: "Home directory is unavailable for global discovery".to_string(),
            });
        }
        return;
    };

    for paths in rules.iter().filter_map(|rule| match rule {
        Rule::HomePaths { paths } => Some(*paths),
        _ => None,
    }) {
        saw_home_rule = true;
        for relative in paths {
            let path = home.join(relative);
            if path.symlink_metadata().is_ok() {
                inspection.listings.push(Listing::Path(path.clone()));
                add_existing_path(target, path, inspection, candidate_paths);
            }
        }
    }

    debug_assert!(
        saw_home_rule || !rules.iter().any(|rule| matches!(rule, Rule::HomePaths { .. }))
    );
}

fn add_existing_path(
    target: TargetId,
    path: PathBuf,
    inspection: &mut Inspection,
    candidate_paths: &mut HashSet<PathBuf>,
) {
    if !candidate_paths.insert(path.clone()) {
        return;
    }

    add_classified_path(target, path, inspection);
}

fn add_classified_path(target: TargetId, path: PathBuf, inspection: &mut Inspection) {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return,
        Err(error) => {
            inspection.diagnostics.push(Diagnostic {
                message: format!("Unable to classify cleanup entry {}: {error}", path.display()),
            });
            return;
        }
    };
    let file_type = metadata.file_type();
    let candidate = if file_type.is_symlink() {
        Candidate::symlink(target, path)
    } else if file_type.is_file() {
        Candidate::file(target, path)
    } else if file_type.is_dir() {
        Candidate::directory(target, path)
    } else {
        inspection.diagnostics.push(Diagnostic {
            message: format!(
                "Unsupported cleanup entry type at {}; the entry was not selected",
                path.display()
            ),
        });
        return;
    };
    inspection.candidates.push(candidate);
}

#[cfg(test)]
fn candidate_path(candidate: &Candidate) -> Option<&Path> {
    match candidate.action() {
        super::Action::RemovePath { path, .. } => Some(path),
        super::Action::RunProcess { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use super::*;
    use crate::cleanup::{Action, EntryKind};
    const TEST_TARGET: TargetId = TargetId::new("test");

    fn current_scope(root: PathBuf) -> Scope {
        Scope::resolve(true, None, root).expect("current scope resolves")
    }

    fn default_scope(home: PathBuf) -> Scope {
        Scope::resolve(false, Some(home), "/working".into()).expect("default scope resolves")
    }

    fn candidate_paths(inspection: &Inspection) -> Vec<PathBuf> {
        inspection.candidates.iter().filter_map(candidate_path).map(Path::to_path_buf).collect()
    }

    #[test]
    fn directory_rule_produces_candidates_and_listings_from_one_inspection() {
        const RULES: &[Rule] =
            &[Rule::DirectoryNames { names: &["node_modules"], parent_marker: None }];
        let temp = TempDir::new().expect("temp directory is created");
        let matched = temp.child("project/node_modules");
        matched.create_dir_all().expect("matched directory exists");
        matched.child("index.js").write_str("cache").expect("cache file exists");

        let scope = current_scope(temp.path().to_path_buf());
        let inspection = inspect_rules(TEST_TARGET, &scope, RULES).expect("inspection succeeds");

        assert_eq!(candidate_paths(&inspection), vec![matched.path().to_path_buf()]);
        assert_eq!(
            inspection.listings,
            vec![Listing::Count { label: "node_modules".to_string(), count: 1 }]
        );
    }

    #[test]
    fn parent_marker_rule_rejects_unowned_directory_names() {
        const RULES: &[Rule] =
            &[Rule::DirectoryNames { names: &["target"], parent_marker: Some("Cargo.toml") }];
        let temp = TempDir::new().expect("temp directory is created");
        let owned = temp.child("crate/target");
        owned.create_dir_all().expect("owned target exists");
        temp.child("crate/Cargo.toml").write_str("[package]").expect("manifest exists");
        temp.child("other/target").create_dir_all().expect("unowned target exists");

        let scope = current_scope(temp.path().to_path_buf());
        let inspection = inspect_rules(TEST_TARGET, &scope, RULES).expect("inspection succeeds");

        assert_eq!(candidate_paths(&inspection), vec![owned.path().to_path_buf()]);
    }

    #[test]
    fn marker_children_rule_reports_only_existing_artifacts() {
        const RULES: &[Rule] = &[Rule::MarkerChildren {
            marker: "Package.swift",
            children: &[".build", ".swiftpm"],
            listing: "SwiftPM Projects (.build, .swiftpm)",
        }];
        let temp = TempDir::new().expect("temp directory is created");
        let package = temp.child("package");
        package.create_dir_all().expect("package exists");
        package.child("Package.swift").write_str("// package").expect("manifest exists");
        let build = package.child(".build");
        build.create_dir_all().expect("build directory exists");

        let scope = current_scope(temp.path().to_path_buf());
        let inspection = inspect_rules(TEST_TARGET, &scope, RULES).expect("inspection succeeds");

        assert_eq!(candidate_paths(&inspection), vec![build.path().to_path_buf()]);
        assert_eq!(
            inspection.listings,
            vec![Listing::Count {
                label: "SwiftPM Projects (.build, .swiftpm)".to_string(),
                count: 1,
            }]
        );
    }

    #[cfg(unix)]
    #[test]
    fn marker_children_classify_file_directory_and_dangling_links_as_links() {
        const RULES: &[Rule] = &[Rule::MarkerChildren {
            marker: "Package.swift",
            children: &["file-link", "directory-link", "dangling-link"],
            listing: "SwiftPM links",
        }];
        let temp = TempDir::new().expect("temp directory is created");
        let package = temp.child("package");
        package.create_dir_all().expect("package exists");
        package.child("Package.swift").write_str("// package").expect("manifest exists");
        let file = temp.child("outside-file");
        file.write_str("outside").expect("outside file exists");
        let directory = temp.child("outside-directory");
        directory.create_dir_all().expect("outside directory exists");
        symlink(file.path(), package.path().join("file-link")).expect("file link exists");
        symlink(directory.path(), package.path().join("directory-link"))
            .expect("directory link exists");
        symlink(temp.path().join("missing"), package.path().join("dangling-link"))
            .expect("dangling link exists");

        let scope = current_scope(temp.path().to_path_buf());
        let inspection = inspect_rules(TEST_TARGET, &scope, RULES).expect("inspection succeeds");

        assert_eq!(inspection.candidates.len(), 3);
        assert!(inspection.candidates.iter().all(|candidate| matches!(
            candidate.action(),
            Action::RemovePath { kind: EntryKind::Symlink, .. }
        )));
    }

    #[cfg(unix)]
    #[test]
    fn directory_name_rule_selects_a_link_without_descending_through_it() {
        const RULES: &[Rule] =
            &[Rule::DirectoryNames { names: &["node_modules"], parent_marker: None }];
        let temp = TempDir::new().expect("temp directory is created");
        let outside = temp.child("outside");
        outside.child("nested/node_modules").create_dir_all().expect("outside tree exists");
        let project = temp.child("project");
        project.create_dir_all().expect("project exists");
        let link = project.child("node_modules");
        symlink(outside.path(), link.path()).expect("directory link exists");

        let scope = current_scope(project.path().to_path_buf());
        let inspection = inspect_rules(TEST_TARGET, &scope, RULES).expect("inspection succeeds");

        assert_eq!(inspection.candidates.len(), 1);
        assert!(matches!(
            inspection.candidates[0].action(),
            Action::RemovePath { kind: EntryKind::Symlink, .. }
        ));
    }

    #[test]
    fn missing_root_is_an_explicit_diagnostic() {
        const RULES: &[Rule] =
            &[Rule::DirectoryNames { names: &["node_modules"], parent_marker: None }];
        let temp = TempDir::new().expect("temp directory is created");
        let missing = temp.path().join("missing");

        let scope = current_scope(missing.clone());
        let inspection = inspect_rules(TEST_TARGET, &scope, RULES).expect("inspection succeeds");

        assert_eq!(
            inspection.diagnostics,
            vec![Diagnostic {
                message: format!("Scan root does not exist: {}", missing.display())
            }]
        );
    }

    #[test]
    fn home_rules_are_excluded_from_current_mode() {
        const RULES: &[Rule] = &[Rule::HomePaths { paths: &["Library/Caches/example"] }];
        let home = TempDir::new().expect("temp home is created");
        let cache = home.child("Library/Caches/example");
        cache.create_dir_all().expect("cache exists");
        let default_scope = default_scope(home.path().to_path_buf());
        let default_inspection =
            inspect_rules(TEST_TARGET, &default_scope, RULES).expect("default inspection succeeds");
        assert_eq!(candidate_paths(&default_inspection), vec![cache.path().to_path_buf()]);

        let current_scope =
            Scope::resolve(true, None, home.path().to_path_buf()).expect("current scope resolves");
        let current_inspection =
            inspect_rules(TEST_TARGET, &current_scope, RULES).expect("current inspection succeeds");
        assert!(current_inspection.candidates.is_empty());
        assert!(current_inspection.listings.is_empty());
    }
}
