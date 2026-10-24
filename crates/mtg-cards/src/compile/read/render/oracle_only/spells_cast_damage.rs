//! Damage that counts the spells you cast: "~ deals X damage to each creature and planeswalker your opponents
//! control, where X is the number of spells you've cast this turn." The renderers have no wording for the count, so it
//! is written as X and named at the end.

use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, TurnTally};

/// Adds the wordings of damage whose amount is the number of spells the controller cast this turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let tally = Amount::Tallied {
        who: Ref::You,
        what: TurnTally::SpellsCast,
    };
    let steps: Vec<Effect> = ability
        .effects()
        .iter()
        .map(|effect| match effect {
            Effect::DealDamage { source, to, amount } if *amount == tally => Effect::DealDamage {
                source: source.clone(),
                to: to.clone(),
                amount: Amount::X,
            },
            other => other.clone(),
        })
        .collect();
    if steps == ability.effects() {
        return;
    }
    let Some(plain) = with_effects(ability, steps) else {
        return;
    };
    for text in variants(&plain) {
        let Some(sentence) = text.strip_suffix('.') else {
            continue;
        };
        let said = format!("{sentence}, where X is the number of spells you've cast this turn.");
        texts.push(said.replacen("creature or planeswalker", "creature and planeswalker", 1));
        texts.push(said);
    }
}
