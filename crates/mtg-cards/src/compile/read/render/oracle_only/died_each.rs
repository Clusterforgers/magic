//! Tokens counted by the creatures that died this turn: "create a Treasure token for each nontoken creature that died this
//! turn." The renderers write "create X Treasure tokens, where X is the number of nontoken creatures that died this turn".

use super::super::super::triggers::article;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, TurnTally};

/// Adds the "for each" wordings of tokens counted by the creatures of every player that died this turn.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let counted = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::CreateToken {
                count: Amount::Tallied {
                    who: Ref::EachPlayer,
                    what: TurnTally::CreaturesDied | TurnTally::NontokenCreaturesDied,
                },
                ..
            }
        )
    });
    if !counted {
        return;
    }
    let said: Vec<String> = texts.iter().filter_map(|text| each_wording(text)).collect();
    texts.extend(said);
}

/// The text with its first "create X ... tokens, where X is the number of ... creatures that died" said per creature.
fn each_wording(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("create X ")?;
    let (name, tail) = rest.split_once(" tokens, where X is the number of ")?;
    let each = tail.replacen("creatures that died", "creature that died", 1);
    Some(format!(
        "{head}create {} {name} token for each {each}",
        article(name)
    ))
}
