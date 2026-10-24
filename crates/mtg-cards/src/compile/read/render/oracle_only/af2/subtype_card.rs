//! A card of a zone that is a subtype: "Return target creature card from your graveyard to your hand. If it's a Zombie
//! card, draw a card." The renderers leave out "card" after the subtype.

use super::super::super::{Form, article, phrase_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Targetable};
use crate::value::Condition;

/// Whether the one target is a card in a zone.
fn card_target(ability: &Ability) -> bool {
    matches!(ability.targets(), [spec] if matches!(&spec.what, Targetable::Object(selector)
        if selector.0.iter().any(|pred| matches!(pred, Pred::In(_)))))
}

/// Adds the wordings of a step on the target card that asks for its subtype and says "card".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !card_target(ability) {
        return;
    }
    for effect in ability.effects() {
        let Effect::If {
            condition:
                Condition::Matches {
                    subject: Ref::Target(0),
                    selector,
                },
            ..
        } = effect
        else {
            continue;
        };
        let Some(phrase) = phrase_text(selector, Form::Subject) else {
            continue;
        };
        let asked = format!("it's {} {phrase}", article(&phrase));
        let said: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(&asked))
            .map(|text| text.replacen(&asked, &format!("{asked} card"), 1))
            .collect();
        texts.extend(said);
    }
}
