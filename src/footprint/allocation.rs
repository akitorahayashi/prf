use std::collections::BTreeMap;
#[cfg(unix)]
use std::collections::HashMap;
#[cfg(unix)]
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::sync::{Arc, Mutex};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use super::{Error, Estimate};

#[cfg(unix)]
const UNIX_BLOCK_BYTES: u64 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootId(usize);

impl RootId {
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    id: RootId,
    path: PathBuf,
}

impl Root {
    pub fn new(id: RootId, path: PathBuf) -> Self {
        Self { id, path }
    }

    #[cfg(test)]
    pub const fn id(&self) -> RootId {
        self.id
    }
}

#[derive(Debug, Clone, Default)]
pub struct Breakdown {
    paths: BTreeMap<RootId, Estimate>,
    #[cfg(test)]
    reported: Estimate,
    total: Estimate,
}

impl Breakdown {
    pub fn path(&self, root: RootId) -> Result<Estimate, Error> {
        self.paths.get(&root).copied().ok_or(Error::InvalidRoot(root.index()))
    }

    #[cfg(test)]
    pub const fn reported(&self) -> Estimate {
        self.reported
    }

    pub const fn total(&self) -> Estimate {
        self.total
    }
}

#[derive(Debug, Clone)]
struct LinkedFile {
    allocated: Estimate,
    link_count: u64,
    occurrences: Vec<Vec<RootId>>,
}

#[derive(Debug, Clone, Default)]
pub struct Index {
    paths: Vec<PathBuf>,
    ordinary: Vec<Estimate>,
    linked_files: Vec<LinkedFile>,
}

impl Index {
    pub fn measure(roots: &[Root]) -> Result<Self, Error> {
        validate_roots(roots)?;
        if roots.is_empty() {
            return Ok(Self::default());
        }

        #[cfg(unix)]
        {
            measure_unix(roots)
        }

        #[cfg(not(unix))]
        {
            let _ = roots;
            Err(Error::UnsupportedPlatform)
        }
    }

    pub fn breakdown<I, R>(&self, roots: I, reported: R) -> Result<Breakdown, Error>
    where
        I: IntoIterator<Item = RootId>,
        R: IntoIterator<Item = Estimate>,
    {
        let selected = self.normalize_selection(roots)?;
        let mut paths = selected
            .iter()
            .map(|root| (*root, self.ordinary[root.index()]))
            .collect::<BTreeMap<_, _>>();

        for linked in &self.linked_files {
            let mut selected_occurrences = 0u64;
            let mut attributed_root = None;

            for owners in &linked.occurrences {
                let owner = owners
                    .iter()
                    .copied()
                    .filter(|root| selected.binary_search(root).is_ok())
                    .min();
                if let Some(owner) = owner {
                    selected_occurrences =
                        selected_occurrences.checked_add(1).ok_or(Error::Overflow)?;
                    attributed_root =
                        Some(attributed_root.map_or(owner, |current: RootId| current.min(owner)));
                }
            }

            if selected_occurrences >= linked.link_count
                && let Some(root) = attributed_root
            {
                let current = paths.get(&root).copied().ok_or(Error::InvalidRoot(root.index()))?;
                paths.insert(root, current.checked_add(linked.allocated)?);
            }
        }

        let path_total = paths.values().copied().try_fold(Estimate::ZERO, Estimate::checked_add)?;
        let reported = reported.into_iter().try_fold(Estimate::ZERO, Estimate::checked_add)?;
        let total = path_total.checked_add(reported)?;

        Ok(Breakdown {
            paths,
            #[cfg(test)]
            reported,
            total,
        })
    }

    fn normalize_selection<I>(&self, roots: I) -> Result<Vec<RootId>, Error>
    where
        I: IntoIterator<Item = RootId>,
    {
        let mut selected = roots.into_iter().collect::<Vec<_>>();
        selected.sort_unstable();
        selected.dedup();

        for root in &selected {
            if root.index() >= self.paths.len() {
                return Err(Error::InvalidRoot(root.index()));
            }
        }

        Ok(maximal_root_ids(&self.paths, selected))
    }
}

fn validate_roots(roots: &[Root]) -> Result<(), Error> {
    for (index, root) in roots.iter().enumerate() {
        if root.id.index() != index {
            return Err(Error::InvalidRoot(root.id.index()));
        }
    }
    Ok(())
}

fn maximal_root_ids(paths: &[PathBuf], mut roots: Vec<RootId>) -> Vec<RootId> {
    roots.sort_unstable_by(|left, right| {
        paths[left.index()].components().cmp(paths[right.index()].components())
    });
    let mut maximal: Vec<RootId> = Vec::with_capacity(roots.len());
    for root in roots {
        if maximal
            .last()
            .is_some_and(|ancestor| paths[root.index()].starts_with(&paths[ancestor.index()]))
        {
            continue;
        }
        maximal.push(root);
    }
    maximal.sort_unstable();
    maximal
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
struct LinkObservation {
    identity: FileIdentity,
    allocated: Estimate,
    link_count: u64,
    owners: Vec<RootId>,
}

#[cfg(unix)]
#[derive(Default)]
struct LocalAllocation {
    ordinary: HashMap<RootId, Estimate>,
    links: Vec<LinkObservation>,
}

#[cfg(unix)]
impl LocalAllocation {
    fn record(&mut self, metadata: &fs::Metadata, mut owners: Vec<RootId>) -> Result<(), Error> {
        let allocated = Estimate::from_bytes(
            metadata.blocks().checked_mul(UNIX_BLOCK_BYTES).ok_or(Error::Overflow)?,
        );
        if allocated == Estimate::ZERO {
            return Ok(());
        }

        owners.sort_unstable();
        owners.dedup();

        if metadata.file_type().is_file() && metadata.nlink() > 1 {
            self.links.push(LinkObservation {
                identity: FileIdentity { device: metadata.dev(), inode: metadata.ino() },
                allocated,
                link_count: metadata.nlink(),
                owners,
            });
            return Ok(());
        }

        for owner in owners {
            let current = self.ordinary.get(&owner).copied().unwrap_or(Estimate::ZERO);
            self.ordinary.insert(owner, current.checked_add(allocated)?);
        }
        Ok(())
    }
}

#[cfg(unix)]
struct AggregateLink {
    allocated: Estimate,
    link_count: u64,
    occurrences: Vec<Vec<RootId>>,
}

#[cfg(unix)]
struct Aggregate {
    ordinary: Vec<Estimate>,
    links: HashMap<FileIdentity, AggregateLink>,
}

#[cfg(unix)]
impl Aggregate {
    fn new(root_count: usize) -> Self {
        Self { ordinary: vec![Estimate::ZERO; root_count], links: HashMap::new() }
    }

    fn merge(&mut self, local: LocalAllocation) -> Result<(), Error> {
        for (root, amount) in local.ordinary {
            self.ordinary[root.index()] = self.ordinary[root.index()].checked_add(amount)?;
        }

        for observation in local.links {
            let linked = self.links.entry(observation.identity).or_insert_with(|| AggregateLink {
                allocated: observation.allocated,
                link_count: observation.link_count,
                occurrences: Vec::new(),
            });
            linked.allocated = linked.allocated.min(observation.allocated);
            linked.link_count = linked.link_count.max(observation.link_count);
            linked.occurrences.push(observation.owners);
        }
        Ok(())
    }
}

#[cfg(unix)]
struct WalkState {
    roots_by_path: HashMap<PathBuf, RootId>,
    aggregate: Mutex<Aggregate>,
    error: Mutex<Option<Error>>,
}

#[cfg(unix)]
impl WalkState {
    fn owners_for(&self, path: &Path, inherited: &[RootId]) -> Vec<RootId> {
        let mut owners = inherited.to_vec();
        if let Some(root) = self.roots_by_path.get(path)
            && !owners.contains(root)
        {
            owners.push(*root);
        }
        owners
    }

    fn merge(&self, local: LocalAllocation) {
        if self.has_error() {
            return;
        }

        let result = self.aggregate.lock().expect("footprint aggregate lock").merge(local);
        if let Err(error) = result {
            self.record_error(error);
        }
    }

    fn record_error(&self, error: Error) {
        let mut stored = self.error.lock().expect("footprint error lock");
        if stored.is_none() {
            *stored = Some(error);
        }
    }

    fn has_error(&self) -> bool {
        self.error.lock().expect("footprint error lock").is_some()
    }
}

#[cfg(unix)]
fn measure_unix(roots: &[Root]) -> Result<Index, Error> {
    let roots_by_path =
        roots.iter().map(|root| (root.path.clone(), root.id)).collect::<HashMap<_, _>>();
    let maximal_roots = maximal_root_ids(
        &roots.iter().map(|root| root.path.clone()).collect::<Vec<_>>(),
        roots.iter().map(|root| root.id).collect(),
    )
    .into_iter()
    .map(|root| roots[root.index()].clone())
    .collect::<Vec<_>>();

    let state = Arc::new(WalkState {
        roots_by_path,
        aggregate: Mutex::new(Aggregate::new(roots.len())),
        error: Mutex::new(None),
    });

    rayon::scope_fifo(|scope| {
        for root in maximal_roots {
            let state = Arc::clone(&state);
            scope.spawn_fifo(move |scope| {
                visit_root(scope, root.path, vec![root.id], state);
            });
        }
    });

    if let Some(error) = state.error.lock().expect("footprint error lock").take() {
        return Err(error);
    }

    let state = Arc::try_unwrap(state).ok().expect("footprint workers completed");
    let aggregate = state.aggregate.into_inner().expect("footprint aggregate lock");
    let linked_files = aggregate
        .links
        .into_values()
        .map(|linked| LinkedFile {
            allocated: linked.allocated,
            link_count: linked.link_count,
            occurrences: linked.occurrences,
        })
        .collect();

    Ok(Index {
        paths: roots.iter().map(|root| root.path.clone()).collect(),
        ordinary: aggregate.ordinary,
        linked_files,
    })
}

#[cfg(unix)]
fn visit_root<'scope>(
    scope: &rayon::ScopeFifo<'scope>,
    path: PathBuf,
    owners: Vec<RootId>,
    state: Arc<WalkState>,
) {
    if state.has_error() {
        return;
    }

    let metadata = match read_metadata(&path) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return,
        Err(error) => {
            state.record_error(error);
            return;
        }
    };

    let owners = state.owners_for(&path, &owners);
    let mut local = LocalAllocation::default();
    if let Err(error) = local.record(&metadata, owners.clone()) {
        state.record_error(error);
        return;
    }
    state.merge(local);

    if metadata.is_dir() {
        visit_directory(scope, path, owners, state);
    }
}

#[cfg(unix)]
fn visit_directory<'scope>(
    scope: &rayon::ScopeFifo<'scope>,
    path: PathBuf,
    owners: Vec<RootId>,
    state: Arc<WalkState>,
) {
    if state.has_error() {
        return;
    }

    let entries = match fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return,
        Err(error) => {
            state.record_error(Error::inspect(path, error));
            return;
        }
    };

    let mut local = LocalAllocation::default();
    for entry in entries {
        if state.has_error() {
            return;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => {
                state.record_error(Error::inspect(&path, error));
                return;
            }
        };
        let child_path = entry.path();
        let metadata = match read_metadata(&child_path) {
            Ok(Some(metadata)) => metadata,
            Ok(None) => continue,
            Err(error) => {
                state.record_error(error);
                return;
            }
        };
        let child_owners = state.owners_for(&child_path, &owners);
        if let Err(error) = local.record(&metadata, child_owners.clone()) {
            state.record_error(error);
            return;
        }

        if metadata.is_dir() {
            let state = Arc::clone(&state);
            scope.spawn_fifo(move |scope| {
                visit_directory(scope, child_path, child_owners, state);
            });
        }
    }
    state.merge(local);
}

#[cfg(unix)]
fn read_metadata(path: &Path) -> Result<Option<fs::Metadata>, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::inspect(path, error)),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs::File;

    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;

    fn allocated(path: &Path) -> u64 {
        fs::symlink_metadata(path).expect("metadata exists").blocks() * UNIX_BLOCK_BYTES
    }

    fn measured(roots: Vec<PathBuf>) -> (Index, Vec<RootId>) {
        let roots = roots
            .into_iter()
            .enumerate()
            .map(|(index, path)| Root::new(RootId::new(index), path))
            .collect::<Vec<_>>();
        let ids = roots.iter().map(Root::id).collect();
        (Index::measure(&roots).expect("measurement succeeds"), ids)
    }

    #[test]
    fn optimized_subset_normalization_matches_a_quadratic_oracle() {
        let paths = vec![
            PathBuf::from("/a"),
            PathBuf::from("/a/child"),
            PathBuf::from("/a/sibling"),
            PathBuf::from("/a-neighbor"),
            PathBuf::from("/z"),
            PathBuf::from("/z/child"),
        ];
        let index = Index {
            paths: paths.clone(),
            ordinary: vec![Estimate::ZERO; paths.len()],
            linked_files: Vec::new(),
        };

        for mask in 0..(1usize << paths.len()) {
            let selected = (0..paths.len())
                .filter(|root| mask & (1 << root) != 0)
                .map(RootId::new)
                .collect::<Vec<_>>();
            let actual = index.normalize_selection(selected.clone()).expect("selection is valid");
            let expected = selected
                .iter()
                .copied()
                .filter(|candidate| {
                    !selected.iter().copied().any(|other| {
                        candidate != &other
                            && paths[candidate.index()].starts_with(&paths[other.index()])
                    })
                })
                .collect::<Vec<_>>();

            assert_eq!(actual, expected, "selection differs for subset mask {mask:#08b}");
        }
    }

    #[test]
    fn breakdown_rejects_unknown_root_lookups() {
        assert!(matches!(Breakdown::default().path(RootId::new(0)), Err(Error::InvalidRoot(0))));
    }

    #[test]
    fn sparse_files_use_allocated_blocks() {
        let temp = TempDir::new().expect("temp directory is created");
        let root = temp.child("cache");
        root.create_dir_all().expect("root exists");
        let sparse = root.child("sparse.bin");
        File::create(sparse.path())
            .expect("sparse file is created")
            .set_len(1024 * 1024 * 1024)
            .expect("logical length is set");
        let (index, roots) = measured(vec![root.path().to_path_buf()]);

        let estimate = index.breakdown(roots, []).expect("footprint is calculated").total().bytes();

        assert_eq!(estimate, allocated(root.path()) + allocated(sparse.path()));
        assert!(estimate < 1024 * 1024 * 1024);
    }

    #[test]
    fn hard_links_inside_selection_count_allocation_once() {
        let temp = TempDir::new().expect("temp directory is created");
        let root = temp.child("cache");
        root.create_dir_all().expect("root exists");
        let first = root.child("first.bin");
        first.write_binary(&[1; 4096]).expect("file exists");
        let second = root.child("second.bin");
        fs::hard_link(first.path(), second.path()).expect("hard link exists");
        let (index, roots) = measured(vec![root.path().to_path_buf()]);

        let estimate = index.breakdown(roots, []).expect("footprint is calculated").total().bytes();

        assert_eq!(estimate, allocated(root.path()) + allocated(first.path()));
    }

    #[test]
    fn hard_link_outside_selection_is_not_reclaimable() {
        let temp = TempDir::new().expect("temp directory is created");
        let root = temp.child("cache");
        root.create_dir_all().expect("root exists");
        let first = root.child("first.bin");
        first.write_binary(&[1; 4096]).expect("file exists");
        fs::hard_link(first.path(), temp.path().join("outside.bin")).expect("outside link exists");
        let (index, roots) = measured(vec![root.path().to_path_buf()]);

        let estimate = index.breakdown(roots, []).expect("footprint is calculated").total().bytes();

        assert_eq!(estimate, allocated(root.path()));
    }

    #[test]
    fn hard_links_across_selected_roots_are_selection_aware() {
        let temp = TempDir::new().expect("temp directory is created");
        let first_root = temp.child("first");
        let second_root = temp.child("second");
        first_root.create_dir_all().expect("first root exists");
        second_root.create_dir_all().expect("second root exists");
        let first = first_root.child("shared.bin");
        first.write_binary(&[1; 4096]).expect("file exists");
        fs::hard_link(first.path(), second_root.path().join("shared.bin"))
            .expect("hard link exists");
        let (index, roots) =
            measured(vec![first_root.path().to_path_buf(), second_root.path().to_path_buf()]);

        let first_only = index
            .breakdown([roots[0]], [])
            .expect("single-root footprint is calculated")
            .total()
            .bytes();
        let both =
            index.breakdown(roots, []).expect("combined footprint is calculated").total().bytes();

        assert_eq!(first_only, allocated(first_root.path()));
        assert_eq!(
            both,
            allocated(first_root.path()) + allocated(second_root.path()) + allocated(first.path())
        );
    }

    #[test]
    fn nested_roots_are_not_counted_twice() {
        let temp = TempDir::new().expect("temp directory is created");
        let parent = temp.child("parent");
        let child = parent.child("child");
        child.create_dir_all().expect("nested root exists");
        let file = child.child("cache.bin");
        file.write_binary(&[1; 4096]).expect("file exists");
        let (index, roots) =
            measured(vec![parent.path().to_path_buf(), child.path().to_path_buf()]);

        let parent_only =
            index.breakdown([roots[0]], []).expect("parent footprint is calculated").total();
        let combined =
            index.breakdown(roots, []).expect("combined footprint is calculated").total();

        assert_eq!(combined, parent_only);
    }

    #[test]
    fn internal_symbolic_links_are_not_followed() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("temp directory is created");
        let root = temp.child("cache");
        root.create_dir_all().expect("root exists");
        let external = temp.child("external.bin");
        external.write_binary(&[1; 4096]).expect("external file exists");
        let link = root.child("external-link");
        symlink(external.path(), link.path()).expect("symbolic link exists");
        let (index, roots) = measured(vec![root.path().to_path_buf()]);

        let estimate = index.breakdown(roots, []).expect("footprint is calculated").total().bytes();

        assert_eq!(estimate, allocated(root.path()) + allocated(link.path()));
    }

    #[test]
    fn missing_roots_are_idempotent() {
        let temp = TempDir::new().expect("temp directory is created");
        let (index, roots) = measured(vec![temp.path().join("missing")]);

        assert_eq!(
            index.breakdown(roots, []).expect("missing footprint succeeds").total(),
            Estimate::ZERO
        );
    }

    #[test]
    fn entry_disappearance_after_directory_enumeration_is_idempotent() {
        let temp = TempDir::new().expect("temp directory is created");
        let file = temp.child("vanishing.bin");
        file.write_binary(&[1; 16]).expect("file exists");
        let entry = fs::read_dir(temp.path())
            .expect("directory is readable")
            .next()
            .expect("entry exists")
            .expect("entry is readable");
        fs::remove_file(file.path()).expect("entry vanishes before metadata");

        assert!(read_metadata(&entry.path()).expect("missing metadata is tolerated").is_none());
    }

    #[test]
    fn reported_estimates_have_an_explicit_additive_basis() {
        let index = Index::default();
        let breakdown = index
            .breakdown([], [Estimate::from_bytes(10), Estimate::from_bytes(20)])
            .expect("reported estimates combine");

        assert_eq!(breakdown.reported(), Estimate::from_bytes(30));
        assert_eq!(breakdown.total(), Estimate::from_bytes(30));
    }
}
