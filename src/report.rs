//! Scan aggregation model shared by the `app` and `output` layers.
//!
//! Owned by neither layer: `app` produces a `ScanReport` and `output` renders it, so the
//! model lives in a neutral leaf module both depend on, keeping the layer graph acyclic.

use std::collections::BTreeMap;

use crate::targets::category::Category;
use crate::targets::item::CleanupItem;

#[derive(Debug, Clone)]
pub struct CategoryReport {
    pub category: Category,
    pub items: Vec<CleanupItem>,
}

impl CategoryReport {
    pub fn new(category: Category) -> Self {
        Self { category, items: Vec::new() }
    }

    pub fn total_size(&self) -> u64 {
        self.items.iter().map(|item| item.size).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ScanReport {
    pub categories: BTreeMap<Category, CategoryReport>,
}

impl ScanReport {
    pub fn new() -> Self {
        Self { categories: BTreeMap::new() }
    }

    pub fn add_item(&mut self, item: CleanupItem) {
        let category = item.category;
        let entry =
            self.categories.entry(category).or_insert_with(|| CategoryReport::new(category));
        entry.items.push(item);
    }

    pub fn total_size(&self) -> u64 {
        self.categories.values().map(CategoryReport::total_size).sum()
    }

    pub fn categories(&self) -> Vec<Category> {
        self.categories.keys().copied().collect()
    }

    pub fn report_for(&self, category: Category) -> Option<&CategoryReport> {
        self.categories.get(&category)
    }

    pub fn subset(&self, categories: &[Category]) -> Self {
        let mut subset = ScanReport::new();
        for category in categories {
            if let Some(report) = self.categories.get(category) {
                subset.categories.insert(*category, report.clone());
            }
        }
        subset
    }

    pub fn is_empty(&self) -> bool {
        self.categories.values().all(CategoryReport::is_empty)
    }
}

impl Default for ScanReport {
    fn default() -> Self {
        ScanReport::new()
    }
}
