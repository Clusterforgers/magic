//! The Verified marker: derived from the check a deck stored when it was saved and the check run now,
//! never kept as a flag (`docs/deck-check.md`, "The marker").

use serde::{Deserialize, Serialize};

use super::{DeckReport, DeckState, Summary};

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

/// What a deck shows to its owner: a state and whether the Verified marker may be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Marker {
    /// `unchecked` without a stored check or after a change; else the worse of the stored and the current state.
    pub state: DeckState,
    /// True exactly when `state` is `verified`.
    pub verified: bool,
    /// A check is stored but the deck, its cards' rules or the engine changed since: "changed since the last check".
    pub changed: bool,
}

/// Derives a deck's marker from the check stored with it and the report of the deck as it is now.
///
/// The deck is `verified` only when the stored check is clean, its deck hash and coverage hash equal
/// the ones computed now, and the report of now is clean too, so a ban or a rule change that left
/// the hashes alone still removes the marker. A deck that changed since its check is `unchecked`; a
/// stored check that was not clean never gives a marker.
pub fn marker(stored: Option<&StoredCheck>, current: &DeckReport) -> Marker {
    let unchecked = |changed| Marker {
        state: DeckState::Unchecked,
        verified: false,
        changed,
    };
    let Some(stored) = stored else {
        return unchecked(false);
    };
    if stored.deck_hash != current.deck_hash || stored.coverage_hash != current.coverage_hash {
        return unchecked(true);
    }
    let ranked = (stored.summary.state.rank(), current.summary.state.rank());
    let (Some(then), Some(now)) = ranked else {
        return unchecked(false);
    };
    let state = if then >= now {
        stored.summary.state
    } else {
        current.summary.state
    };
    Marker {
        state,
        verified: state == DeckState::Verified,
        changed: false,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the marker follows the stored check and the report of now: a clean check of an unchanged deck is verified, any change of the deck or of coverage removes it, and a check that was not clean never gives it.
    use rstest::rstest;

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
            },
        }
    }

    fn stored(deck: &str, coverage: &str, state: DeckState) -> StoredCheck {
        StoredCheck::of(&report(deck, coverage, state), "2026-10-19T12:00:00Z")
    }

    #[rstest]
    #[case::clean_and_unchanged(Some(("d", "c", DeckState::Verified)), ("d", "c", DeckState::Verified), DeckState::Verified, false)]
    #[case::one_edit(Some(("d", "c", DeckState::Verified)), ("d2", "c", DeckState::Verified), DeckState::Unchecked, true)]
    #[case::the_engine_or_the_rules_changed(Some(("d", "c", DeckState::Verified)), ("d", "c2", DeckState::Verified), DeckState::Unchecked, true)]
    #[case::never_checked(None, ("d", "c", DeckState::Verified), DeckState::Unchecked, false)]
    #[case::manual_cards_stay_manual(Some(("d", "c", DeckState::ManualPlay)), ("d", "c", DeckState::ManualPlay), DeckState::ManualPlay, false)]
    #[case::blocked_stays_blocked(Some(("d", "c", DeckState::Blocked)), ("d", "c", DeckState::Blocked), DeckState::Blocked, false)]
    #[case::a_ban_since_the_check(Some(("d", "c", DeckState::Verified)), ("d", "c", DeckState::Blocked), DeckState::Blocked, false)]
    #[case::manual_since_the_check(Some(("d", "c", DeckState::Verified)), ("d", "c", DeckState::ManualPlay), DeckState::ManualPlay, false)]
    #[case::a_check_that_was_not_clean_is_not_cleared_by_now(Some(("d", "c", DeckState::ManualPlay)), ("d", "c", DeckState::Verified), DeckState::ManualPlay, false)]
    #[case::a_stored_unchecked_is_nothing(Some(("d", "c", DeckState::Unchecked)), ("d", "c", DeckState::Verified), DeckState::Unchecked, false)]
    fn the_marker_is_derived_from_both_checks(
        #[case] before: Option<(&str, &str, DeckState)>,
        #[case] now: (&str, &str, DeckState),
        #[case] state: DeckState,
        #[case] changed: bool,
    ) {
        let before = before.map(|(deck, coverage, state)| stored(deck, coverage, state));
        let found = marker(before.as_ref(), &report(now.0, now.1, now.2));
        assert_eq!(found.state, state);
        assert_eq!(found.changed, changed);
        assert_eq!(found.verified, state == DeckState::Verified);
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
