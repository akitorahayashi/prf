use byte_unit::{Byte, UnitType};

use crate::cleanup::{ActionEstimate, EstimateSummary};

pub fn format_bytes(size: u64) -> String {
    if size == 0 {
        "0 B".to_string()
    } else {
        let adjusted = Byte::from_u64(size).get_appropriate_unit(UnitType::Decimal);
        format!("{adjusted:#.2}")
    }
}

pub fn format_action_estimate(estimate: ActionEstimate) -> String {
    match estimate {
        ActionEstimate::Known(estimate) => format_bytes(estimate.bytes()),
        ActionEstimate::Unestimated => "unknown".to_string(),
    }
}

pub fn format_estimate_compact(summary: EstimateSummary) -> String {
    match (summary.known().bytes(), summary.unestimated_actions()) {
        (known, 0) => format_bytes(known),
        (0, _) => "unknown".to_string(),
        (known, _) => format!("{} + unknown", format_bytes(known)),
    }
}

pub fn format_estimate_detail(summary: EstimateSummary) -> String {
    let unestimated = summary.unestimated_actions();
    match (summary.known().bytes(), unestimated) {
        (known, 0) => format_bytes(known),
        (0, count) => format!(
            "unknown ({count} unestimated {})",
            if count == 1 { "action" } else { "actions" }
        ),
        (known, count) => format!(
            "{} known + {count} unestimated {}",
            format_bytes(known),
            if count == 1 { "action" } else { "actions" }
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::footprint::Estimate;

    use super::*;

    #[test]
    fn detail_distinguishes_unknown_from_known_zero() {
        assert_eq!(format_estimate_detail(EstimateSummary::ZERO), "0 B");
        assert_eq!(
            format_estimate_detail(EstimateSummary::new(Estimate::ZERO, 1)),
            "unknown (1 unestimated action)"
        );
    }
}
