//! A trigger that works only for a card that was kicked, and a count of the times it was kicked: "When this creature
//! enters, if it was kicked, ..." (the pronoun is the card's) and "you gain 2 life for each time it was kicked".

use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, Condition};

/// The pronouns a card uses for itself.
const PRONOUNS: [&str; 3] = ["it", "he", "she"];

/// Adds the wordings of an "if it was kicked" after the event and of a gain of life for each kick.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if let Ability::Triggered {
        intervening_if: Some(Condition::Kicked),
        ..
    } = ability
    {
        let mut plain = ability.clone();
        if let Ability::Triggered { intervening_if, .. } = &mut plain {
            *intervening_if = None;
        }
        let said: Vec<String> = variants(&plain)
            .iter()
            .filter_map(|text| text.split_once(", "))
            .flat_map(|(event, rest)| {
                PRONOUNS.map(|pronoun| format!("{event}, if {pronoun} was kicked, {rest}"))
            })
            .collect();
        texts.extend(said);
    }
    texts.extend(life_per_kick(ability));
}

/// "When ~ enters, you gain 2 life for each time it was kicked." for a gain of life of a factor of the kicks.
fn life_per_kick(ability: &Ability) -> Vec<String> {
    let Ability::Triggered { effects, .. } = ability else {
        return Vec::new();
    };
    let [
        Effect::GainLife {
            who: Ref::You,
            amount: Amount::Times { by, of },
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if **of != Amount::Kicks {
        return Vec::new();
    }
    let mut plain = ability.clone();
    if let Ability::Triggered { effects, .. } = &mut plain {
        *effects = vec![Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(*by),
        }];
    }
    variants(&plain)
        .iter()
        .filter_map(|text| text.strip_suffix('.'))
        .map(|text| format!("{text} for each time it was kicked."))
        .collect()
}
