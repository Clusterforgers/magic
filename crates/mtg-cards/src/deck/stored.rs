//! The check a save stores with a deck (`docs/deck-check.md`, "The deck state").

use serde::{Deserialize, Serialize};

use super::{DeckReport, Summary};

/// What a save stores of a check: enough to tell later whether the deck, the rules or the engine
/// changed, and what the result was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoredCheck {
    /// The [`DeckReport::deck_hash`] of the checked deck.
    pub deck_hash: String,
    /// The [`DeckReport::coverage_hash`] of its cards.
    pub coverage_hash: String,
    /// When the check ran, as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
    pub checked_at: String,
    /// The counts and the state the check gave.
    pub summary: Summary,
}

impl StoredCheck {
    /// The part of `report` a save keeps, stamped with the time it ran.
    pub fn of(report: &DeckReport, checked_at: impl Into<String>) -> StoredCheck {
        StoredCheck {
            deck_hash: report.deck_hash.clone(),
            coverage_hash: report.coverage_hash.clone(),
            checked_at: checked_at.into(),
            summary: report.summary.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a stored check keeps the hashes, the time and the summary of the report and reads back unchanged.
    use super::*;
    use crate::deck::{Counts, DeckState};

    fn report(deck: &str, coverage: &str, state: DeckState) -> DeckReport {
        DeckReport {
            deck_hash: deck.to_owned(),
            coverage_hash: coverage.to_owned(),
            format: "commander".to_owned(),
            legality: Vec::new(),
            cards: Vec::new(),
            summary: Summary {
                format: "commander".to_owned(),
                state,
                counts: Counts::default(),
                zones: Vec::new(),
                issues: 0,
                to_read: 0,
            },
        }
    }

    #[test]
    fn a_stored_check_keeps_the_hashes_the_time_and_the_summary_of_the_report() {
        let report = report("d", "c", DeckState::ManualPlay);
        let kept = StoredCheck::of(&report, "2026-01-02T03:04:05Z");
        assert_eq!(
            serde_json::to_value(&kept).unwrap(),
            serde_json::json!({
                "deck_hash": "d",
                "coverage_hash": "c",
                "checked_at": "2026-01-02T03:04:05Z",
                "summary": serde_json::to_value(&report.summary).unwrap(),
            })
        );
        let again: StoredCheck =
            serde_json::from_value(serde_json::to_value(&kept).unwrap()).unwrap();
        assert_eq!(again, kept);
    }
}
