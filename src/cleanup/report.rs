use std::collections::BTreeMap;

use super::candidate::Candidate;
use super::target::{Target, TargetId};

#[derive(Debug, Clone)]
pub struct TargetReport {
    pub target: TargetId,
    pub candidates: Vec<Candidate>,
}

impl TargetReport {
    pub fn new(target: TargetId) -> Self {
        Self { target, candidates: Vec::new() }
    }

    pub fn total_size(&self) -> u64 {
        self.candidates.iter().map(Candidate::estimated_size).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    reports: BTreeMap<TargetId, TargetReport>,
}

impl ScanReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_candidate(&mut self, candidate: Candidate) {
        let target = candidate.target;
        self.reports
            .entry(target)
            .or_insert_with(|| TargetReport::new(target))
            .candidates
            .push(candidate);
    }

    pub fn total_size(&self) -> u64 {
        self.reports.values().map(TargetReport::total_size).sum()
    }

    pub fn target_ids(&self) -> Vec<TargetId> {
        self.reports.keys().copied().collect()
    }

    pub fn report_for(&self, target: TargetId) -> Option<&TargetReport> {
        self.reports.get(&target)
    }

    pub fn subset(&self, targets: &[&Target]) -> Self {
        let mut subset = Self::new();
        for target in targets {
            if let Some(report) = self.reports.get(&target.id()) {
                subset.reports.insert(target.id(), report.clone());
            }
        }
        subset
    }

    pub fn candidates_for(&self, targets: &[&Target]) -> Vec<Candidate> {
        targets
            .iter()
            .filter_map(|target| self.report_for(target.id()))
            .flat_map(|report| report.candidates.clone())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.reports.values().all(TargetReport::is_empty)
    }
}
