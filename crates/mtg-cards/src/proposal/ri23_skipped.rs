//! An ability the checker cannot read back must still be one the engine runs: an unwritten ability that uses a
//! primitive the engine lacks is a problem, so a draft is never valid yet does nothing in a game.

use std::collections::BTreeSet;

use super::{ClauseReport, ClauseState};
use crate::card::Card;
use crate::load::LoadError;
use crate::support::Caps;
use crate::validate::used_primitives;

/// The problems of the abilities of `proposed` that stand for a clause no writer reads back and use a primitive
/// missing from `caps`: one per ability, at its path in `file`.
pub(super) fn ri23_skipped(
    proposed: &Card,
    clauses: &[ClauseReport],
    caps: &Caps,
    file: &str,
) -> Vec<LoadError> {
    let unwritten: BTreeSet<(usize, usize)> = clauses
        .iter()
        .filter_map(|clause| match clause.state {
            ClauseState::Unwritten { ability, .. } => Some((clause.face, ability)),
            _ => None,
        })
        .collect();
    unwritten
        .into_iter()
        .filter_map(|(face, index)| {
            let ability = proposed.faces.get(face)?.abilities.get(index)?;
            let missing = caps.missing(&used_primitives(ability));
            (!missing.is_empty()).then(|| LoadError {
                file: file.to_owned(),
                path: format!("faces[{face}].abilities[{index}]"),
                message: format!(
                    "the engine does not run {} yet and no writer reads the ability back, so it would do nothing: \
                     leave the clause by hand (`Manual`) or write it with what the engine plays",
                    missing.join(", ")
                ),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: an unwritten ability is a problem exactly when the engine lacks a primitive it uses, one per ability whatever
    //! the number of its clauses, and a written or open clause is never one.
    use rstest::rstest;

    use super::*;
    use crate::card::Span;
    use crate::compile::ClauseKind;

    fn card(abilities: &str) -> Card {
        let json = format!(
            r#"{{"oracle_id":"x","name":"X","impl_status":"Full","faces":[{{"name":"X","oracle_fingerprint":"f","abilities":{abilities}}}]}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    fn clause(state: ClauseState) -> ClauseReport {
        ClauseReport {
            face: 0,
            span: Span { start: 0, end: 0 },
            text: String::new(),
            kind: ClauseKind::Static,
            state,
        }
    }

    fn unwritten(ability: usize) -> ClauseReport {
        clause(ClauseState::Unwritten {
            ability,
            construct: String::new(),
        })
    }

    const MODAL: &str = r#"[{"Replacement":{"when":{"LifeGain":{}},"by":{"Instead":[{"Modal":{"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":1}}]}]}}]}}}]"#;
    const MAY: &str = r#"[{"Replacement":{"when":{"LifeGain":{}},"by":{"Instead":[{"May":{"effects":[{"Draw":{}}]}}]}}}]"#;

    #[rstest]
    #[case::skipped_unwritten(MODAL, vec![unwritten(0)], 1)]
    #[case::skipped_twice_one_problem(MODAL, vec![unwritten(0), unwritten(0)], 1)]
    #[case::skipped_but_read_back(MODAL, vec![clause(ClauseState::Reads { ability: 0 })], 0)]
    #[case::open(MODAL, vec![clause(ClauseState::Open)], 0)]
    #[case::run_unwritten(MAY, vec![unwritten(0)], 0)]
    fn an_unwritten_ability_the_engine_skips_is_a_problem(
        #[case] abilities: &str,
        #[case] clauses: Vec<ClauseReport>,
        #[case] problems: usize,
    ) {
        let caps = Caps::from_names(used_primitives(
            &serde_json::from_str(r#"{"Replacement":{"when":{"LifeGain":{}},"by":{"Instead":[{"May":{"effects":[{"Draw":{}}]}}]}}}"#).unwrap(),
        ));
        let found = ri23_skipped(&card(abilities), &clauses, &caps, "cards/x.json");
        assert_eq!(found.len(), problems, "{found:?}");
        if let Some(problem) = found.first() {
            assert!(
                problem.message.contains("ReplaceBy.Instead.Modal"),
                "{problem}"
            );
        }
    }
}
