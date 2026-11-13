//! The riders of a cast permission that are replacement effects (CR 614.1c): the permanent a spell cast through it
//! becomes enters tapped or with a counter, and the spell is exiled rather than put into its owner's graveyard.

use mtg_cards::modify::CastRider;
use mtg_cards::refs::ZoneKind;
use mtg_cards::trigger::ReplaceBy;
use mtg_cards::value::Amount;

use super::{Action, Candidate, Origin, Pass, Slot, Unit};
use crate::state::GameState;
use crate::zones::Zone;
use mtg_core::ids::ObjectId;

/// What the player ordering the replacements reads for the rider at `index` of the spell `spell`.
pub(super) fn r401_rider_words(state: &GameState, spell: ObjectId, index: usize) -> &'static str {
    match state
        .object(spell)
        .and_then(|spell| spell.cast_riders().get(index))
    {
        Some(CastRider::EntersTapped) => "~ was cast through a permission: it enters tapped.",
        Some(CastRider::EntersWithCounter(_)) => {
            "~ was cast through a permission: it enters with a counter on it."
        }
        _ => "~ was cast through a permission: exile it instead.",
    }
}

impl Pass<'_> {
    /// The riders of the spell the unit moves that replace the move and have not been applied to it: the entering riders
    /// as it enters, the exile as it would go from the stack to a graveyard.
    pub(super) fn r401_rider_candidates(&self, slot: &Slot, unit: &Unit) -> Vec<Candidate> {
        let (object, entering) = match unit {
            Unit::Enter { object, .. } => (*object, true),
            Unit::Move {
                object,
                from: Zone::Stack,
                to: Zone::Graveyard(_),
                ..
            } => (*object, false),
            _ => return Vec::new(),
        };
        let Some(spell) = self
            .state
            .object(object)
            .filter(|o| o.zone() == Zone::Stack)
        else {
            return Vec::new();
        };
        spell
            .cast_riders()
            .iter()
            .enumerate()
            .filter_map(|(index, rider)| {
                let by = match (rider, entering) {
                    (CastRider::EntersTapped, true) => ReplaceBy::Tapped,
                    (CastRider::EntersWithCounter(counter), true) => ReplaceBy::WithCounters {
                        counter: counter.clone(),
                        count: Amount::one(),
                        tapped: false,
                    },
                    (CastRider::ExiledInstead, false) => ReplaceBy::MoveTo(ZoneKind::Exile),
                    _ => return None,
                };
                let origin = Origin::R401Rider(index);
                (!slot.applied.contains(&origin)).then(|| Candidate {
                    origin,
                    object: spell.reference(),
                    controller: spell.controller(),
                    action: Action::Replace(by),
                    // An effect of the permission's source, ordered with the others that apply (CR 616.1).
                    is_self: false,
                })
            })
            .collect()
    }
}
