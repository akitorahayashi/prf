use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use crate::error::AppError;
use crate::footprint::{Estimate, Index};

use super::candidate::Candidate;
use super::plan::{RemovalCatalog, RemovalPlan};
use super::target::{Target, TargetId};

#[derive(Debug, Clone)]
pub struct CandidateReport {
    pub candidate: Candidate,
    estimate: Estimate,
}

impl CandidateReport {
    pub const fn estimate(&self) -> Estimate {
        self.estimate
    }
}

#[derive(Debug, Clone)]
pub struct TargetReport {
    pub target: TargetId,
    pub candidates: Vec<CandidateReport>,
    estimate: Estimate,
}

impl TargetReport {
    pub const fn estimate(&self) -> Estimate {
        self.estimate
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ScanReport {
    candidates: Arc<Vec<Candidate>>,
    catalog: Arc<RemovalCatalog>,
    footprint: Arc<Index>,
    reports: BTreeMap<TargetId, TargetReport>,
    selected: Vec<usize>,
    estimate: Estimate,
}

impl ScanReport {
    pub fn empty() -> Self {
        Self {
            candidates: Arc::new(Vec::new()),
            catalog: Arc::new(RemovalCatalog::default()),
            footprint: Arc::new(Index::default()),
            reports: BTreeMap::new(),
            selected: Vec::new(),
            estimate: Estimate::ZERO,
        }
    }

    pub fn build(
        candidates: Vec<Candidate>,
        catalog: RemovalCatalog,
        footprint: Index,
        targets: &[&Target],
    ) -> Result<Self, AppError> {
        let candidates = Arc::new(candidates);
        let catalog = Arc::new(catalog);
        let footprint = Arc::new(footprint);
        let target_ids = targets.iter().map(|target| target.id()).collect::<Vec<_>>();
        Self::view(candidates, catalog, footprint, &target_ids)
    }

    fn view(
        candidates: Arc<Vec<Candidate>>,
        catalog: Arc<RemovalCatalog>,
        footprint: Arc<Index>,
        targets: &[TargetId],
    ) -> Result<Self, AppError> {
        let selected_targets = targets.iter().copied().collect::<HashSet<_>>();
        let selected = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                selected_targets.contains(&candidate.target).then_some(index)
            })
            .collect::<Vec<_>>();
        let plan = catalog.plan(&candidates, &selected)?;
        let breakdown = footprint.breakdown(plan.roots(), plan.reported())?;
        let mut estimates = Contributions::new(&selected);
        for path in plan.paths() {
            estimates.assign(path.attribution(), breakdown.path(path.root()))?;
        }
        for process in plan.processes() {
            estimates.assign(process.candidate(), process.estimate())?;
        }

        let mut reports = BTreeMap::new();
        for target in targets {
            let indices = candidates
                .iter()
                .enumerate()
                .filter_map(|(index, candidate)| (candidate.target == *target).then_some(index))
                .collect::<Vec<_>>();
            if indices.is_empty() {
                continue;
            }

            let candidate_reports = indices
                .iter()
                .map(|index| CandidateReport {
                    candidate: candidates[*index].clone(),
                    estimate: estimates.get(*index),
                })
                .collect::<Vec<_>>();
            let estimate = candidate_reports
                .iter()
                .map(CandidateReport::estimate)
                .try_fold(Estimate::ZERO, Estimate::checked_add)?;
            reports.insert(
                *target,
                TargetReport { target: *target, candidates: candidate_reports, estimate },
            );
        }

        Ok(Self { candidates, catalog, footprint, reports, selected, estimate: breakdown.total() })
    }

    pub const fn estimate(&self) -> Estimate {
        self.estimate
    }

    pub fn target_ids(&self) -> Vec<TargetId> {
        self.reports.keys().copied().collect()
    }

    pub fn report_for(&self, target: TargetId) -> Option<&TargetReport> {
        self.reports.get(&target)
    }

    pub fn subset(&self, targets: &[&Target]) -> Result<Self, AppError> {
        let target_ids = targets.iter().map(|target| target.id()).collect::<Vec<_>>();
        Self::view(
            Arc::clone(&self.candidates),
            Arc::clone(&self.catalog),
            Arc::clone(&self.footprint),
            &target_ids,
        )
    }

    pub fn estimate_for(&self, targets: &[TargetId]) -> Result<Estimate, AppError> {
        let targets = targets.iter().copied().collect::<HashSet<_>>();
        let selected = self
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| targets.contains(&candidate.target).then_some(index))
            .collect::<Vec<_>>();
        let plan = self.catalog.plan(&self.candidates, &selected)?;
        Ok(self.footprint.breakdown(plan.roots(), plan.reported())?.total())
    }

    pub fn removal_plan(&self) -> Result<RemovalPlan, AppError> {
        self.catalog.plan(&self.candidates, &self.selected)
    }

    pub fn footprint(&self) -> &Index {
        &self.footprint
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
}

struct Contributions {
    values: BTreeMap<usize, Estimate>,
}

impl Contributions {
    fn new(indices: &[usize]) -> Self {
        Self { values: indices.iter().copied().map(|index| (index, Estimate::ZERO)).collect() }
    }

    fn assign(&mut self, index: usize, estimate: Estimate) -> Result<(), AppError> {
        let current = self.values.get(&index).copied().ok_or_else(|| {
            AppError::Cleanup("footprint attribution references an unknown candidate".to_string())
        })?;
        self.values.insert(index, current.checked_add(estimate)?);
        Ok(())
    }

    fn get(&self, index: usize) -> Estimate {
        self.values.get(&index).copied().unwrap_or(Estimate::ZERO)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::MetadataExt;

    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;
    use crate::cleanup::{Candidate, ScopeSupport};
    use crate::cleanup::{Discovery, Inspection, Scope};

    const FIRST: TargetId = TargetId::new("first");
    const SECOND: TargetId = TargetId::new("second");

    fn no_inspection(_: TargetId, _: &Scope) -> Result<Inspection, AppError> {
        Ok(Inspection::default())
    }

    static FIRST_TARGET: Target =
        Target::new(FIRST, "First", ScopeSupport::AllModes, Discovery::Inspector(no_inspection));
    static SECOND_TARGET: Target =
        Target::new(SECOND, "Second", ScopeSupport::AllModes, Discovery::Inspector(no_inspection));

    fn build(candidates: Vec<Candidate>) -> ScanReport {
        let catalog = RemovalCatalog::new(&candidates).expect("catalog is valid");
        let footprint =
            Index::measure(&catalog.measurement_roots()).expect("footprint is measured");
        ScanReport::build(candidates, catalog, footprint, &[&FIRST_TARGET, &SECOND_TARGET])
            .expect("report is built")
    }

    #[test]
    fn target_and_overall_estimates_use_selected_path_unions() {
        let temp = TempDir::new().expect("temp directory is created");
        let parent = temp.child("parent");
        let child = parent.child("child");
        child.create_dir_all().expect("child exists");
        child.child("cache.bin").write_binary(&[1; 4096]).expect("file exists");
        let report = build(vec![
            Candidate::directory(FIRST, parent.path().to_path_buf()),
            Candidate::directory(SECOND, child.path().to_path_buf()),
        ]);

        let first = report.report_for(FIRST).expect("first report").estimate();
        let second_contribution = report.report_for(SECOND).expect("second report").estimate();
        let second_alone = report.estimate_for(&[SECOND]).expect("standalone estimate");

        assert!(first >= second_alone);
        assert_eq!(second_contribution, Estimate::ZERO);
        assert_eq!(report.estimate(), first);
        assert_eq!(first.checked_add(second_contribution).unwrap(), report.estimate());
        assert_eq!(report.subset(&[&SECOND_TARGET]).unwrap().estimate(), second_alone);
    }

    #[test]
    fn candidate_contributions_sum_to_their_target_estimate() {
        let temp = TempDir::new().expect("temp directory is created");
        let first = temp.child("first");
        let second = temp.child("second");
        first.write_binary(&[1; 4096]).expect("first file exists");
        fs::hard_link(first.path(), second.path()).expect("hard link exists");
        let report = build(vec![
            Candidate::file(FIRST, first.path().to_path_buf()),
            Candidate::file(FIRST, second.path().to_path_buf()),
        ]);
        let target = report.report_for(FIRST).expect("target report");
        let candidate_total = target
            .candidates
            .iter()
            .map(CandidateReport::estimate)
            .try_fold(Estimate::ZERO, Estimate::checked_add)
            .expect("candidate total is valid");

        assert_eq!(candidate_total, target.estimate());
        assert_eq!(target.estimate().bytes(), fs::metadata(first.path()).unwrap().blocks() * 512);
    }

    #[test]
    fn candidate_disappearance_after_planning_is_idempotent() {
        let temp = TempDir::new().expect("temp directory is created");
        let root = temp.child("cache");
        root.create_dir_all().expect("root exists");
        let candidates = vec![Candidate::directory(FIRST, root.path().to_path_buf())];
        let catalog = RemovalCatalog::new(&candidates).expect("catalog is valid");
        fs::remove_dir(root.path()).expect("root disappears before measurement");
        let footprint =
            Index::measure(&catalog.measurement_roots()).expect("missing root is tolerated");

        let report = ScanReport::build(candidates, catalog, footprint, &[&FIRST_TARGET])
            .expect("report is built");

        assert_eq!(report.estimate(), Estimate::ZERO);
    }
}
