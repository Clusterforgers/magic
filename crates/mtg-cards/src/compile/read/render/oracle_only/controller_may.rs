//! The controller of the creature of an event may draw: "Whenever a creature deals combat damage to one of your
//! opponents, its controller may draw a card." The renderers word the draw of the controller without the choice and the
//! opponent as "an opponent".

use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings of a trigger whose effect is a draw the controller of the object of the event may make.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let controller = Ref::ControllerOf(Box::new(Ref::Triggering));
    let [
        Effect::May {
            who: chooser,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [Effect::Draw { who, count }] = inner.as_slice() else {
        return;
    };
    if *chooser != controller || *who != controller || !count.is_one() {
        return;
    }
    let Some(bare) = with_effects(ability, inner.clone()) else {
        return;
    };
    for text in variants(&bare) {
        if !text.ends_with(", that permanent's controller draws a card.") {
            continue;
        }
        let said = text.replacen(
            ", that permanent's controller draws a card.",
            ", its controller may draw a card.",
            1,
        );
        texts.push(said.replacen(" to an opponent,", " to one of your opponents,", 1));
        texts.push(said);
    }
}
