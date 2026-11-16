//! "As this creature transforms into ~, ..." (`ReplaceWhen::R6603Transforms`, CR 701.28, 614.1c): the replacement is
//! an ability of the face the permanent turns up, so it is read from the permanent as that face, and its effects
//! (`ReplaceBy::AsEnters`) happen in front of the transform in the same batch. A permanent put onto the battlefield
//! transformed enters (an `Entered` event) and does not transform (CR 712.14a), so nothing applies then.

use mtg_cards::ability::Ability;
use mtg_cards::trigger::ReplaceWhen;

use super::{Pass, Source, Unit};
use crate::ability_reader::abilities_of;
use crate::characteristics::controller_of_object;
use crate::events::Event;
use crate::state::GameState;

/// Whether `event` turns a permanent over to a face that prints an "as this transforms" replacement, which the step must
/// look at although no permanent in play has a replacement effect now.
pub(super) fn r6603_turns_up_replacement(state: &GameState, event: &Event) -> bool {
    let Event::Transformed { object, face } = event else {
        return false;
    };
    state
        .object(*object)
        .and_then(|card| card.def.faces.get(face.map_or(0, usize::from)))
        .is_some_and(|shown| {
            crate::ability_reader::printed_rules(shown).any(|rules| {
                matches!(
                    &**rules,
                    Ability::Replacement {
                        when: ReplaceWhen::R6603Transforms { .. },
                        ..
                    }
                )
            })
        })
}

impl Pass<'_> {
    /// The "as this transforms" replacements of the permanent a transform turns over, read from the face it turns up.
    pub(super) fn r6603_turned_up(&self, unit: &Unit) -> Vec<Source> {
        let Unit::Transform { object, face } = unit else {
            return Vec::new();
        };
        let mut view = self.state.clone();
        let Some(permanent) = view
            .objects
            .get_mut(object)
            .and_then(|card| card.permanent.as_mut())
        else {
            return Vec::new();
        };
        permanent.face = *face;
        let Some(card) = view.object(*object) else {
            return Vec::new();
        };
        let controller = controller_of_object(&view, card);
        let reference = card.reference();
        abilities_of(&view, *object)
            .into_iter()
            .filter(|(_, rules)| {
                matches!(
                    &**rules,
                    Ability::Replacement {
                        when: ReplaceWhen::R6603Transforms { .. },
                        ..
                    }
                )
            })
            .map(|(address, ability)| Source {
                address,
                ability,
                controller,
                object: reference,
                copied: false,
            })
            .collect()
    }
}
