//! A mass return from every graveyard to the battlefield under the owners: "Return all artifact and enchantment cards
//! from all graveyards to the battlefield under their owners' control."

use super::super::super::selectors::{Form, phrase_text};
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, ZoneKind};

/// Adds the wording of a spell that puts the cards of a kind in any graveyard onto the battlefield under their owners.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { effects, .. } = ability else {
        return;
    };
    let [
        Effect::Move {
            what: Ref::Each(selector),
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters,
                },
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let any_graveyard = selector.0.contains(&Pred::In(ZoneKind::Graveyard))
        && !selector
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::OwnedBy(_)));
    if !counters.is_empty() || !any_graveyard {
        return;
    }
    let Some(phrase) = phrase_text(selector, Form::All) else {
        return;
    };
    let Some(cards) = phrase.strip_suffix(" from a graveyard") else {
        return;
    };
    let sentence = format!(
        "return all {cards} from all graveyards to the battlefield under their owners' control"
    );
    texts.extend(with_effects(ability, &sentence));
}
