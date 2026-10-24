//! Clue tokens made by investigating several times: "Investigate three times." The renderers write the tokens.

use super::super::super::count_word;
use crate::ability::Ability;
use crate::effect::{Effect, TokenRef};
use crate::value::Amount;

/// Adds the wordings of a spell or ability that creates two or more Clue tokens as an investigation repeated.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for effect in ability.effects() {
        let Effect::CreateToken {
            token: TokenRef::Named(name),
            count: Amount::Fixed(number),
            ..
        } = effect
        else {
            continue;
        };
        let Some(word) = u32::try_from(*number)
            .ok()
            .filter(|number| *number >= 2)
            .and_then(count_word)
        else {
            continue;
        };
        if name != "clue" {
            continue;
        }
        let into = format!("Investigate {word} times");
        let said: Vec<String> = [". Create ", ". You create "]
            .iter()
            .flat_map(|opening| {
                let from = format!("{opening}{word} Clue tokens");
                let into = format!(". {into}");
                texts
                    .iter()
                    .filter(|text| text.contains(&from))
                    .map(|text| text.replacen(&from, &into, 1))
                    .collect::<Vec<_>>()
            })
            .collect();
        texts.extend(said);
    }
}
