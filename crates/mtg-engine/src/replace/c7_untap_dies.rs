//! "If that creature would die this turn, exile it instead": the death shields of the permanents an effect named
//! (`Rule::DiesInstead`, CR 614.1a, 611.2c).

use mtg_cards::trigger::ReplaceBy;

use super::{Action, Candidate, Origin, Pass, Slot, Unit};
use crate::object::GameObject;
use crate::rules::Rule;
use crate::zones::Zone;

impl Pass<'_> {
    /// The death shields that send the permanent of the slot elsewhere as it would go from the battlefield to a
    /// graveyard, each once; a shield applies to every death it covers while it lasts and is never used up.
    pub(super) fn c7_untap_dies(&self, slot: &Slot) -> Vec<Candidate> {
        let Unit::Move {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(_),
            ..
        } = self.unit(&slot.item)
        else {
            return Vec::new();
        };
        let Some(current) = self.state.object(object).map(GameObject::counter) else {
            return Vec::new();
        };
        self.shields
            .iter()
            .filter_map(|effect| {
                let Rule::DiesInstead { objects, to } = &effect.rule else {
                    return None;
                };
                let origin = Origin::Shield(effect.id);
                let covered = objects
                    .iter()
                    .any(|named| named.id == object && named.counter == current);
                (covered && !slot.applied.contains(&origin)).then_some(Candidate {
                    origin,
                    object: effect.source?,
                    controller: effect.controller,
                    action: Action::Replace(ReplaceBy::MoveTo(*to)),
                    is_self: false,
                })
            })
            .collect()
    }
}
