use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::footprint::{Basis, Estimate, Root, RootId};

use super::{Action, Candidate, EntryKind};

#[derive(Debug, Clone)]
struct CatalogRoot {
    id: RootId,
    path: PathBuf,
    kind: EntryKind,
}

#[derive(Debug, Clone, Default)]
pub struct RemovalCatalog {
    candidates: Vec<Candidate>,
    roots: Vec<CatalogRoot>,
    candidate_roots: Vec<Option<RootId>>,
}

impl RemovalCatalog {
    pub fn new(candidates: Vec<Candidate>) -> Result<Self, AppError> {
        let mut roots: Vec<CatalogRoot> = Vec::new();
        let mut roots_by_path: HashMap<PathBuf, usize> = HashMap::new();
        let mut candidate_roots = Vec::with_capacity(candidates.len());

        for candidate in &candidates {
            match (&candidate.action, candidate.basis()) {
                (Action::RemovePath { path, kind }, Basis::Allocated) => {
                    let resolved = normalize_terminal_entry(path)?;

                    if let Some(index) = roots_by_path.get(&resolved).copied() {
                        if roots[index].kind != *kind {
                            return Err(AppError::Cleanup(format!(
                                "conflicting entry kinds for {}",
                                resolved.display()
                            )));
                        }
                        candidate_roots.push(Some(roots[index].id));
                        continue;
                    }

                    let id = RootId::new(roots.len());
                    roots_by_path.insert(resolved.clone(), roots.len());
                    roots.push(CatalogRoot { id, path: resolved, kind: *kind });
                    candidate_roots.push(Some(id));
                }
                (Action::RunProcess { .. }, Basis::Reported(_)) => candidate_roots.push(None),
                (Action::RemovePath { .. }, Basis::Reported(_))
                | (Action::RunProcess { .. }, Basis::Allocated) => {
                    return Err(AppError::Cleanup(
                        "candidate action and footprint basis do not match".to_string(),
                    ));
                }
            }
        }

        Ok(Self { candidates, roots, candidate_roots })
    }

    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    pub fn measurement_roots(&self) -> Vec<Root> {
        self.roots.iter().map(|root| Root::new(root.id, root.path.clone())).collect()
    }

    pub fn plan(&self, selected: &[usize]) -> Result<RemovalPlan, AppError> {
        let mut selected = selected.to_vec();
        selected.sort_unstable();
        selected.dedup();
        if selected.iter().any(|index| *index >= self.candidates.len()) {
            return Err(AppError::Cleanup(
                "removal plan references an unknown candidate".to_string(),
            ));
        }

        let mut selected_roots =
            selected.iter().filter_map(|index| self.candidate_roots[*index]).collect::<Vec<_>>();
        selected_roots.sort_unstable();
        selected_roots.dedup();
        let all_selected_roots = selected_roots.clone();
        selected_roots.retain(|candidate| {
            !all_selected_roots.iter().copied().any(|other| {
                *candidate != other
                    && is_strict_descendant(
                        &self.roots[candidate.index()].path,
                        &self.roots[other.index()].path,
                    )
            })
        });

        let paths = selected_roots
            .into_iter()
            .map(|root_id| {
                let root = &self.roots[root_id.index()];
                let candidates = selected
                    .iter()
                    .copied()
                    .filter(|index| {
                        self.candidate_roots[*index].is_some_and(|candidate_root| {
                            let candidate_path = &self.roots[candidate_root.index()].path;
                            candidate_path == &root.path || candidate_path.starts_with(&root.path)
                        })
                    })
                    .collect::<Vec<_>>();
                let attribution = candidates
                    .iter()
                    .copied()
                    .find(|index| self.candidate_roots[*index] == Some(root_id))
                    .expect("selected root has a source candidate");

                PathRemoval {
                    root: root_id,
                    path: root.path.clone(),
                    kind: root.kind,
                    candidates,
                    attribution,
                }
            })
            .collect();

        let mut processes = Vec::new();
        for index in selected {
            let candidate = &self.candidates[index];
            match (&candidate.action, candidate.basis()) {
                (Action::RunProcess { label, program, args }, Basis::Reported(estimate)) => {
                    processes.push(ProcessRemoval {
                        candidate: index,
                        label,
                        program,
                        args,
                        estimate,
                    })
                }
                (Action::RemovePath { .. }, Basis::Allocated) => {}
                _ => {
                    return Err(AppError::Cleanup(
                        "candidate action and footprint basis do not match".to_string(),
                    ));
                }
            }
        }

        Ok(RemovalPlan { paths, processes })
    }
}

fn is_strict_descendant(child: &Path, ancestor: &Path) -> bool {
    child != ancestor && child.starts_with(ancestor)
}

fn normalize_terminal_entry(path: &Path) -> Result<PathBuf, AppError> {
    let Some(name) = path.file_name() else {
        return match std::fs::canonicalize(path) {
            Ok(path) => Ok(path),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(path.to_path_buf()),
            Err(error) => Err(AppError::Io(error)),
        };
    };
    let Some(parent) = path.parent() else {
        return Ok(path.to_path_buf());
    };

    match std::fs::canonicalize(parent) {
        Ok(parent) => Ok(parent.join(name)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(AppError::Io(error)),
    }
}

#[derive(Debug, Clone)]
pub struct PathRemoval {
    root: RootId,
    path: PathBuf,
    kind: EntryKind,
    candidates: Vec<usize>,
    attribution: usize,
}

impl PathRemoval {
    pub const fn root(&self) -> RootId {
        self.root
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn kind(&self) -> EntryKind {
        self.kind
    }

    pub fn candidates(&self) -> &[usize] {
        &self.candidates
    }

    pub const fn attribution(&self) -> usize {
        self.attribution
    }
}

#[derive(Debug, Clone)]
pub struct ProcessRemoval {
    candidate: usize,
    label: &'static str,
    program: &'static str,
    args: &'static [&'static str],
    estimate: Estimate,
}

impl ProcessRemoval {
    pub const fn candidate(&self) -> usize {
        self.candidate
    }

    pub const fn label(&self) -> &'static str {
        self.label
    }

    pub const fn program(&self) -> &'static str {
        self.program
    }

    pub const fn args(&self) -> &'static [&'static str] {
        self.args
    }

    pub const fn estimate(&self) -> Estimate {
        self.estimate
    }
}

#[derive(Debug, Clone, Default)]
pub struct RemovalPlan {
    paths: Vec<PathRemoval>,
    processes: Vec<ProcessRemoval>,
}

impl RemovalPlan {
    pub fn paths(&self) -> &[PathRemoval] {
        &self.paths
    }

    pub fn processes(&self) -> &[ProcessRemoval] {
        &self.processes
    }

    pub fn action_count(&self) -> usize {
        self.paths.len() + self.processes.len()
    }

    pub fn roots(&self) -> impl Iterator<Item = RootId> + '_ {
        self.paths.iter().map(PathRemoval::root)
    }

    pub fn reported(&self) -> impl Iterator<Item = Estimate> + '_ {
        self.processes.iter().map(ProcessRemoval::estimate)
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;
    use crate::cleanup::TargetId;

    const TARGET: TargetId = TargetId::new("test");

    #[cfg(unix)]
    #[test]
    fn plan_merges_duplicates_and_ancestor_aliases_and_omits_descendants() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp directory is created");
        let parent = temp.child("parent");
        let child = parent.child("child");
        child.create_dir_all().expect("nested directory exists");
        let alias = temp.child("alias");
        symlink(parent.path(), alias.path()).expect("alias exists");
        let candidates = vec![
            Candidate::directory(TARGET, parent.path().to_path_buf()),
            Candidate::directory(TARGET, parent.path().to_path_buf()),
            Candidate::directory(TARGET, alias.path().join("child")),
            Candidate::directory(TARGET, child.path().to_path_buf()),
        ];
        let catalog = RemovalCatalog::new(candidates).expect("catalog is valid");

        let plan = catalog.plan(&[0, 1, 2, 3]).expect("plan is built");

        assert_eq!(plan.paths().len(), 1);
        assert_eq!(plan.paths()[0].path(), parent.path().canonicalize().unwrap());
        assert_eq!(plan.paths()[0].candidates(), &[0, 1, 2, 3]);
    }

    #[cfg(unix)]
    #[test]
    fn terminal_symlink_is_not_canonicalized_to_its_target() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp directory is created");
        let outside = temp.child("outside");
        outside.create_dir_all().expect("outside directory exists");
        let link = temp.child("cache-link");
        symlink(outside.path(), link.path()).expect("link exists");
        let catalog =
            RemovalCatalog::new(vec![Candidate::symlink(TARGET, link.path().to_path_buf())])
                .expect("catalog is valid");

        let plan = catalog.plan(&[0]).expect("plan is built");

        assert_eq!(
            plan.paths()[0].path(),
            link.path().parent().unwrap().canonicalize().unwrap().join("cache-link")
        );
        assert_eq!(plan.paths()[0].kind(), EntryKind::Symlink);
    }

    #[test]
    fn plan_rejects_conflicting_kinds_for_one_physical_path() {
        let temp = TempDir::new().expect("temp directory is created");
        let path = temp.child("cache");
        path.create_dir_all().expect("directory exists");
        let candidates = vec![
            Candidate::directory(TARGET, path.path().to_path_buf()),
            Candidate::file(TARGET, path.path().to_path_buf()),
        ];

        assert!(matches!(
            RemovalCatalog::new(candidates),
            Err(AppError::Cleanup(message)) if message.contains("conflicting entry kinds")
        ));
    }

    #[test]
    fn missing_paths_remain_idempotent_plan_roots() {
        let temp = TempDir::new().expect("temp directory is created");
        let missing = temp.path().join("missing");
        let candidates = vec![Candidate::directory(TARGET, missing.clone())];
        let catalog = RemovalCatalog::new(candidates).expect("catalog is valid");

        let plan = catalog.plan(&[0]).expect("plan is built");

        assert_eq!(
            plan.paths()[0].path(),
            missing.parent().unwrap().canonicalize().unwrap().join("missing")
        );
    }

    #[test]
    fn plan_rejects_an_unknown_candidate_index() {
        let catalog = RemovalCatalog::default();

        assert!(matches!(
            catalog.plan(&[0]),
            Err(AppError::Cleanup(message))
                if message.contains("references an unknown candidate")
        ));
    }
}
