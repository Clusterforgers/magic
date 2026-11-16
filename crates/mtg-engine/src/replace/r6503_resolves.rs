//! "Exile that card instead of putting it into your graveyard as it resolves": the shields of the spells an effect named
//! that replace the move of each into a graveyard as it resolves (`Rule::R6503ResolvesInstead`, CR 608.2n, 614.1a), never
//! the move of a spell that is countered or fizzles.

use super::{Action, Candidate, Origin, Pass, Slot, Unit};
use crate::object::GameObject;
use crate::rules::Rule;
use crate::zones::Zone;

impl Pass<'_> {
    /// The shields that replace the move of the resolved spell of the slot into its owner's graveyard, each once.
    pub(super) fn r6503_resolves_instead(&self, slot: &Slot) -> Vec<Candidate> {
        let Unit::Move {
            object,
            from: Zone::Stack,
            to: Zone::Graveyard(_),
            resolved: true,
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
                let Rule::R6503ResolvesInstead { objects, by } = &effect.rule else {
                    return None;
                };
                let origin = Origin::Shield(effect.id);
                let covered = objects
                    .iter()
                    .any(|named| named.id == object && named.counter == current);
                if !covered || slot.applied.contains(&origin) {
                    return None;
                }
                Some(Candidate {
                    origin,
                    object: effect.source?,
                    controller: effect.controller,
                    action: Action::Replace(by.clone()),
                    is_self: false,
                })
            })
            .collect()
    }
}
