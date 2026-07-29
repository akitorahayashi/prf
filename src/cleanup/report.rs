use std::collections::BTreeMap;
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
    pub candidates: Vec<CandidateReport>,
    estimate: Estimate,
}

impl TargetReport {
    pub const fn estimate(&self) -> Estimate {
        self.estimate
    }
}

#[derive(Debug, Clone)]
pub struct ScanReport {
    catalog: Arc<RemovalCatalog>,
    footprint: Arc<Index>,
    reports: BTreeMap<TargetId, TargetReport>,
    standalone_estimates: BTreeMap<TargetId, Estimate>,
    plan: RemovalPlan,
    estimate: Estimate,
}

impl ScanReport {
    pub fn empty() -> Self {
        Self {
            catalog: Arc::new(RemovalCatalog::default()),
            footprint: Arc::new(Index::default()),
            reports: BTreeMap::new(),
            standalone_estimates: BTreeMap::new(),
            plan: RemovalPlan::default(),
            estimate: Estimate::ZERO,
        }
    }

    pub fn build(
        catalog: RemovalCatalog,
        footprint: Index,
        targets: &[&Target],
    ) -> Result<Self, AppError> {
        let catalog = Arc::new(catalog);
        let footprint = Arc::new(footprint);
        let target_ids = targets.iter().map(|target| target.id()).collect::<Vec<_>>();
        Self::view(catalog, footprint, &target_ids)
    }

    fn view(
        catalog: Arc<RemovalCatalog>,
        footprint: Arc<Index>,
        targets: &[TargetId],
    ) -> Result<Self, AppError> {
        let candidates = catalog.candidates();
        let mut indices_by_target =
            targets.iter().copied().map(|target| (target, Vec::new())).collect::<BTreeMap<_, _>>();
        let mut selected = Vec::new();
        for (index, candidate) in candidates.iter().enumerate() {
            if let Some(indices) = indices_by_target.get_mut(&candidate.target()) {
                indices.push(index);
                selected.push(index);
            }
        }
        let plan = catalog.plan(&selected)?;
        let breakdown = footprint.breakdown(plan.roots(), plan.reported())?;
        let mut estimates = Contributions::new(candidates.len(), &selected);
        for path in plan.paths() {
            estimates.assign(path.attribution(), breakdown.path(path.root())?)?;
        }
        for process in plan.processes() {
            estimates.assign(process.candidate(), process.estimate())?;
        }

        let mut reports = BTreeMap::new();
        let mut standalone_estimates = BTreeMap::new();
        for (target, indices) in &indices_by_target {
            let target_plan = catalog.plan(indices)?;
            let standalone =
                footprint.breakdown(target_plan.roots(), target_plan.reported())?.total();
            standalone_estimates.insert(*target, standalone);
            if indices.is_empty() {
                continue;
            }

            let candidate_reports = indices
                .iter()
                .map(|index| {
                    Ok(CandidateReport {
                        candidate: candidates[*index].clone(),
                        estimate: estimates.get(*index)?,
                    })
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            let estimate = candidate_reports
                .iter()
                .map(CandidateReport::estimate)
                .try_fold(Estimate::ZERO, Estimate::checked_add)?;
            reports.insert(*target, TargetReport { candidates: candidate_reports, estimate });
        }

        Ok(Self {
            catalog,
            footprint,
            reports,
            standalone_estimates,
            plan,
            estimate: breakdown.total(),
        })
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
        Self::view(Arc::clone(&self.catalog), Arc::clone(&self.footprint), &target_ids)
    }

    pub fn standalone_estimate(&self, target: TargetId) -> Result<Estimate, AppError> {
        self.standalone_estimates.get(&target).copied().ok_or_else(|| {
            AppError::Cleanup("standalone estimate references an unknown report target".to_string())
        })
    }

    pub const fn removal_plan(&self) -> &RemovalPlan {
        &self.plan
    }

    pub fn footprint(&self) -> &Index {
        &self.footprint
    }

    pub fn is_empty(&self) -> bool {
        self.plan.action_count() == 0
    }
}

struct Contributions {
    values: Vec<Option<Estimate>>,
}

impl Contributions {
    fn new(candidate_count: usize, indices: &[usize]) -> Self {
        let mut values = vec![None; candidate_count];
        for index in indices {
            values[*index] = Some(Estimate::ZERO);
        }
        Self { values }
    }

    fn assign(&mut self, index: usize, estimate: Estimate) -> Result<(), AppError> {
        let value = self.values.get_mut(index).and_then(Option::as_mut).ok_or_else(|| {
            AppError::Cleanup("footprint attribution references an unknown candidate".to_string())
        })?;
        *value = value.checked_add(estimate)?;
        Ok(())
    }

    fn get(&self, index: usize) -> Result<Estimate, AppError> {
        self.values.get(index).copied().flatten().ok_or_else(|| {
            AppError::Cleanup("footprint report references an unknown candidate".to_string())
        })
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
        let catalog = RemovalCatalog::new(candidates).expect("catalog is valid");
        let footprint =
            Index::measure(&catalog.measurement_roots()).expect("footprint is measured");
        ScanReport::build(catalog, footprint, &[&FIRST_TARGET, &SECOND_TARGET])
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
        let second_alone = report.standalone_estimate(SECOND).expect("standalone estimate");

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
        let catalog = RemovalCatalog::new(candidates).expect("catalog is valid");
        fs::remove_dir(root.path()).expect("root disappears before measurement");
        let footprint =
            Index::measure(&catalog.measurement_roots()).expect("missing root is tolerated");

        let report =
            ScanReport::build(catalog, footprint, &[&FIRST_TARGET]).expect("report is built");

        assert_eq!(report.estimate(), Estimate::ZERO);
    }

    #[test]
    fn contribution_storage_rejects_unselected_candidate_indices() {
        let mut contributions = Contributions::new(2, &[0]);

        assert!(matches!(
            contributions.get(1),
            Err(AppError::Cleanup(message)) if message.contains("unknown candidate")
        ));
        assert!(matches!(
            contributions.assign(1, Estimate::ZERO),
            Err(AppError::Cleanup(message)) if message.contains("unknown candidate")
        ));
    }
}
