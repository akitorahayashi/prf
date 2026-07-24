use std::sync::Arc;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar};
use rayon::prelude::*;

use crate::cleanup::{Inspection, RemovalCatalog, ScanReport, Scope, Target};
use crate::error::AppError;
use crate::footprint::Index;
use crate::output::messages;
use crate::output::progress::{discovery_spinner_style, footprint_spinner_style};
use crate::output::report::{print_diagnostics, print_list_results, print_scan_report};

pub struct ScanOptions {
    pub targets: Vec<&'static Target>,
    pub scope: Scope,
    pub verbose: bool,
}

pub fn execute(options: ScanOptions) -> Result<ScanReport, AppError> {
    let progress = Arc::new(MultiProgress::new());
    let report = scan_targets(&options.targets, &options.scope, &progress)?;
    print_scan_report(&report, &options.targets, options.verbose, options.scope.home())?;
    Ok(report)
}

pub fn list_targets(options: ScanOptions) -> Result<(), AppError> {
    list_targets_with(options, |targets, inspections, home| {
        print_diagnostics(inspections)?;
        print_list_results(targets, inspections, home)
    })
}

fn list_targets_with<F>(options: ScanOptions, render: F) -> Result<(), AppError>
where
    F: FnOnce(&[&Target], &[Inspection], Option<&std::path::Path>) -> Result<(), AppError>,
{
    let inspections: Result<Vec<Inspection>, AppError> =
        options.targets.par_iter().map(|target| target.inspect(&options.scope)).collect();
    let inspections = inspections?;
    render(&options.targets, &inspections, options.scope.home())
}

pub fn scan_targets(
    targets: &[&Target],
    scope: &Scope,
    progress: &Arc<MultiProgress>,
) -> Result<ScanReport, AppError> {
    if targets.is_empty() {
        return Ok(ScanReport::empty());
    }

    let discovery_style = Arc::new(discovery_spinner_style());
    let discovery_progress = Arc::clone(progress);
    let inspections: Result<Vec<Inspection>, AppError> = targets
        .par_iter()
        .map(|target| {
            let spinner = discovery_progress.add(ProgressBar::new_spinner());
            spinner.set_style((*discovery_style).clone());
            spinner.enable_steady_tick(Duration::from_millis(100));
            spinner.set_message(messages::discovering(target.display_name()));

            let inspection = target.inspect(scope);
            spinner.finish_and_clear();
            if let Ok(result) = &inspection {
                discovery_progress.println(messages::discovery_complete(
                    target.display_name(),
                    result.candidates.len(),
                ))?;
            }
            inspection
        })
        .collect();
    let inspections = inspections?;

    print_diagnostics(&inspections)?;
    let candidates =
        inspections.into_iter().flat_map(|inspection| inspection.candidates).collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(ScanReport::empty());
    }

    let total_items = candidates.len();
    let footprint_spinner = progress.add(ProgressBar::new_spinner());
    footprint_spinner.set_style(footprint_spinner_style());
    footprint_spinner.enable_steady_tick(Duration::from_millis(100));
    footprint_spinner.set_message(messages::calculating_footprint(total_items));
    let measurement = (|| {
        let catalog = RemovalCatalog::new(candidates)?;
        let footprint = Index::measure(&catalog.measurement_roots())?;
        Ok::<_, AppError>((catalog, footprint))
    })();
    footprint_spinner.finish_and_clear();
    let (catalog, footprint) = measurement?;
    progress.println(messages::footprint_calculation_complete(total_items))?;

    ScanReport::build(catalog, footprint, targets)
}

#[cfg(all(test, unix))]
mod tests {
    use std::io::ErrorKind;
    use std::path::PathBuf;

    use super::*;
    use crate::cleanup::{Candidate, Discovery, ScopeSupport, TargetId};

    const TARGET_ID: TargetId = TargetId::new("list-test");
    const UNMEASURABLE: &str = "/dev/null/prf-list-candidate";

    fn inspect_without_measurement(
        target: TargetId,
        _scope: &Scope,
    ) -> Result<Inspection, AppError> {
        Ok(Inspection {
            candidates: vec![Candidate::directory(target, PathBuf::from(UNMEASURABLE))],
            listings: vec![crate::cleanup::Listing::Detail(
                "Deterministic list candidate".to_string(),
            )],
            diagnostics: Vec::new(),
        })
    }

    static TARGET: Target = Target::new(
        TARGET_ID,
        "List test",
        ScopeSupport::AllModes,
        Discovery::Inspector(inspect_without_measurement),
    );

    #[test]
    fn list_flow_does_not_measure_discovered_candidates() {
        let precondition = std::fs::symlink_metadata(UNMEASURABLE)
            .expect_err("/dev/null cannot contain a child path");
        assert_eq!(precondition.kind(), ErrorKind::NotADirectory);
        let scope =
            Scope::resolve(&[PathBuf::from("/unused")], false, None, PathBuf::from("/unused"))
                .expect("explicit scope resolves");

        list_targets_with(
            ScanOptions { targets: vec![&TARGET], scope, verbose: false },
            |_targets, inspections, _home| {
                assert_eq!(inspections[0].candidates.len(), 1);
                Ok(())
            },
        )
        .expect("list flow succeeds without footprint measurement");
    }
}
