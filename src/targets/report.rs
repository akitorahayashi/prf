use std::collections::{BTreeMap, BTreeSet};

use super::category::Category;
use super::item::{CleanupAction, CleanupItem, ExternalAction, ItemKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategoryStatus {
    Ready,
    Clean,
    Unavailable(String),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct CategoryReport {
    pub category: Category,
    pub status: CategoryStatus,
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub categories: BTreeMap<Category, CategoryReport>,
    pub candidates: Vec<CleanupItem>,
}

impl ScanReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_complete(&mut self, category: Category, items: Vec<CleanupItem>) {
        let status = if items.is_empty() { CategoryStatus::Clean } else { CategoryStatus::Ready };
        self.categories.insert(category, CategoryReport { category, status });
        self.candidates.extend(items);
    }

    pub fn record_unavailable(&mut self, category: Category, reason: String) {
        self.categories.insert(
            category,
            CategoryReport { category, status: CategoryStatus::Unavailable(reason) },
        );
    }

    pub fn record_failed(&mut self, category: Category, reason: String) {
        self.categories
            .insert(category, CategoryReport { category, status: CategoryStatus::Failed(reason) });
        self.candidates.retain(|item| !item.has_category(category));
    }

    pub fn total_size(&self) -> u64 {
        candidate_total_size(&self.items_for_categories(&self.categories()))
    }

    pub fn category_total_size(&self, category: Category) -> u64 {
        candidate_total_size(&self.items_for_categories(&[category]))
    }

    pub fn category_item_count(&self, category: Category) -> usize {
        self.items_for_categories(&[category]).len()
    }

    pub fn categories(&self) -> Vec<Category> {
        self.categories.keys().copied().collect()
    }

    pub fn report_for(&self, category: Category) -> Option<&CategoryReport> {
        self.categories.get(&category)
    }

    pub fn items_for_categories(&self, categories: &[Category]) -> Vec<CleanupItem> {
        let selected = self
            .candidates
            .iter()
            .filter(|item| categories.iter().any(|category| item.has_category(*category)))
            .cloned()
            .map(|mut item| {
                item.retain_categories(categories);
                item
            })
            .collect();
        normalize_candidates(selected)
    }

    pub fn has_candidates_for(&self, categories: &[Category]) -> bool {
        self.candidates
            .iter()
            .any(|item| categories.iter().any(|category| item.has_category(*category)))
    }

    pub fn subset(&self, categories: &[Category]) -> Self {
        let selected_categories = self
            .categories
            .iter()
            .filter(|(category, _)| categories.contains(category))
            .map(|(category, report)| (*category, report.clone()))
            .collect();
        Self { categories: selected_categories, candidates: self.items_for_categories(categories) }
    }

    pub fn ready_categories(&self, requested: &[Category]) -> Vec<Category> {
        requested
            .iter()
            .copied()
            .filter(|category| {
                self.report_for(*category)
                    .is_some_and(|report| report.status == CategoryStatus::Ready)
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

fn normalize_candidates(mut candidates: Vec<CleanupItem>) -> Vec<CleanupItem> {
    candidates.sort_by(|left, right| match (&left.action, &right.action) {
        (CleanupAction::Filesystem(left), CleanupAction::Filesystem(right)) => left
            .path
            .components()
            .count()
            .cmp(&right.path.components().count())
            .then_with(|| left.path.cmp(&right.path)),
        (CleanupAction::Filesystem(_), CleanupAction::External(_)) => std::cmp::Ordering::Less,
        (CleanupAction::External(_), CleanupAction::Filesystem(_)) => std::cmp::Ordering::Greater,
        (CleanupAction::External(left), CleanupAction::External(right)) => left.cmp(right),
    });

    let mut normalized: Vec<CleanupItem> = Vec::new();
    for candidate in candidates {
        if let Some(existing) =
            normalized.iter_mut().find(|existing| contains_candidate(existing, &candidate))
        {
            existing.merge_categories(&candidate);
            continue;
        }
        normalized.push(candidate);
    }

    normalized
}

fn contains_candidate(existing: &CleanupItem, candidate: &CleanupItem) -> bool {
    match (&existing.action, &candidate.action) {
        (CleanupAction::Filesystem(existing), CleanupAction::Filesystem(candidate)) => {
            existing.path == candidate.path
                || (existing.kind == ItemKind::Directory
                    && candidate.path.starts_with(&existing.path))
        }
        (CleanupAction::External(existing), CleanupAction::External(candidate)) => {
            existing == candidate
        }
        _ => false,
    }
}

pub fn candidate_total_size(items: &[CleanupItem]) -> u64 {
    let mut seen = BTreeSet::new();
    let mut total = 0u64;

    for item in items {
        match item.external_action() {
            Some(ExternalAction::DockerPrune) => {
                total = total.saturating_add(item.size);
            }
            None => {
                for allocation in item.allocations() {
                    if seen.insert(allocation.identity) {
                        total = total.saturating_add(allocation.bytes);
                    }
                }
            }
        }
    }

    total
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::targets::item::{Allocation, FileIdentity, PathAuthority};

    fn directory(category: Category, path: &str) -> CleanupItem {
        let path = PathBuf::from(path);
        CleanupItem::directory(
            category,
            path,
            PathAuthority::LocalRoot {
                path: PathBuf::from("/"),
                identity: FileIdentity { device: 1, inode: 1 },
            },
        )
    }

    #[test]
    fn selection_normalizes_only_selected_categories() {
        let mut report = ScanReport::new();
        report.record_complete(
            Category::Xcode,
            vec![directory(Category::Xcode, "/workspace/DerivedData")],
        );
        report.record_complete(
            Category::Nodejs,
            vec![directory(Category::Nodejs, "/workspace/DerivedData/node_modules")],
        );

        let node_items = report.items_for_categories(&[Category::Nodejs]);
        assert_eq!(node_items.len(), 1);
        assert_eq!(
            node_items[0].path(),
            Some(std::path::Path::new("/workspace/DerivedData/node_modules"))
        );

        let all_items = report.items_for_categories(&[Category::Xcode, Category::Nodejs]);
        assert_eq!(all_items.len(), 1);
        assert_eq!(all_items[0].path(), Some(std::path::Path::new("/workspace/DerivedData")));
        assert!(all_items[0].has_category(Category::Nodejs));
    }

    #[test]
    fn totals_count_hard_link_identity_once() {
        let allocation = Allocation { identity: FileIdentity { device: 1, inode: 2 }, bytes: 4096 };
        let mut first = directory(Category::Python, "/workspace/.venv");
        first.set_allocations(vec![allocation]);
        let mut second = directory(Category::Nodejs, "/workspace/node_modules");
        second.set_allocations(vec![allocation]);

        let mut report = ScanReport::new();
        report.record_complete(Category::Python, vec![first]);
        report.record_complete(Category::Nodejs, vec![second]);

        assert_eq!(report.total_size(), 4096);
    }

    #[test]
    fn zero_size_item_remains_a_cleanup_candidate() {
        let mut report = ScanReport::new();
        report.record_complete(
            Category::Nodejs,
            vec![directory(Category::Nodejs, "/workspace/node_modules")],
        );

        assert_eq!(report.total_size(), 0);
        assert!(report.has_candidates_for(&[Category::Nodejs]));
    }
}
