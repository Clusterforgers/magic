//! "If that creature would die this turn, exile it instead", "If it would leave the battlefield, exile it instead": the
//! shields of the permanents and spells an effect named (`Rule::DiesInstead`, CR 614.1a, 611.2c).

use mtg_cards::trigger::ReplaceBy;

use super::{Action, Candidate, Origin, Pass, Slot, Unit};
use crate::object::GameObject;
use crate::resolve::zone_kind;
use crate::rules::Rule;
use crate::zones::Zone;

impl Pass<'_> {
    /// The shields that send the permanent or spell of the slot elsewhere as it would go from the battlefield or the
    /// stack to a graveyard, or (`leaves`) from the battlefield to another zone than theirs, each once; a shield applies
    /// to every move it covers while it lasts and is never used up.
    pub(super) fn c7_untap_dies(&self, slot: &Slot) -> Vec<Candidate> {
        let Unit::Move {
            object,
            from: from @ (Zone::Battlefield | Zone::Stack),
            to: destination,
            countered: was_countered,
            ..
        } = self.unit(&slot.item)
        else {
            return Vec::new();
        };
        let dies = matches!(destination, Zone::Graveyard(_));
        // Phasing out is no zone change (CR 702.26d).
        let leaves_to = (from == Zone::Battlefield && destination != Zone::PhasedOut)
            .then(|| zone_kind(destination));
        let Some(current) = self.state.object(object).map(GameObject::counter) else {
            return Vec::new();
        };
        self.shields
            .iter()
            .filter_map(|effect| {
                let Rule::DiesInstead {
                    objects,
                    to,
                    countered,
                    cast,
                    leaves,
                } = &effect.rule
                else {
                    return None;
                };
                // A move already bound for the shield's zone is left as it is (CR 614.1a).
                let moves = if *leaves {
                    leaves_to.is_some_and(|kind| kind != *to)
                } else {
                    dies
                };
                if !moves {
                    return None;
                }
                let origin = Origin::Shield(effect.id);
                if *countered && !was_countered {
                    return None;
                }
                // A card named in another zone is cast as the next object (CR 400.7): "if that spell would be put into a graveyard".
                let cast_from_there = from == Zone::Stack;
                let covered = objects.iter().any(|named| {
                    named.id == object
                        && (named.counter == current
                            || (*cast && cast_from_there && named.counter + 1 == current))
                });
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
