//! The permanents tapped to pay an activated ability's cost, logged for the turn with the ability's source
//! (`TallyEvent::R110TappedForCost`): "destroy all Merfolk tapped this turn to pay for its abilities".

use mtg_cards::ability::Ability;
use mtg_cards::value::TallyEvent;

use crate::abilities::PutOnStack;
use crate::choose::Target;
use crate::parts::{Kind, Part};
use crate::state::GameState;

/// Logs each permanent picked for a part of the cost that taps permanents ("Tap an untapped Merfolk you control"), in the
/// order the cost prints its picking parts; an ability with no such part logs nothing.
pub(crate) fn r110_record_cost_taps(state: &mut GameState, put: &PutOnStack, rules: &Ability) {
    let Ability::Activated { cost, .. } = rules else {
        return;
    };
    let picks = put.choices.cost_picks();
    let mut at = 0;
    let mut tapped = Vec::new();
    for item in &cost.0 {
        let Ok(Some(part)) = Part::read(item) else {
            continue;
        };
        let pick = match &part {
            Part::Pick(pick) => pick,
            Part::X(inner) => match &**inner {
                Part::Pick(pick) => pick,
                _ => continue,
            },
            _ => continue,
        };
        if matches!(pick.kind, Kind::Tap | Kind::TapPower) {
            tapped.extend(picks.get(at).into_iter().flatten().copied());
        }
        at += 1;
    }
    for object in tapped {
        state.record(
            TallyEvent::R110TappedForCost,
            Some(put.controller),
            (object, Some(Target::Object(put.source))),
            1,
        );
    }
}
