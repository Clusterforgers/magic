//! A group that attacks a player: "Whenever two or more creatures you control attack a player, target attacking
//! creature without flying gains flying until end of turn." The renderers leave out the player and put the duration
//! first.

use crate::ability::Ability;
use crate::trigger::TriggerPattern;

/// Adds the wordings of the attack of a group at a player with the duration at the end.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::GroupAttacks { .. },
        ..
    } = ability
    else {
        return;
    };
    let said: Vec<String> = texts.iter().filter_map(|text| at_a_player(text)).collect();
    texts.extend(said);
}

/// "... attack, Until end of turn, target X gains Y." as "... attack a player, target X gains Y until end of turn."
fn at_a_player(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(" attack, Until end of turn, ")?;
    let body = rest.strip_suffix('.')?;
    Some(format!("{head} attack a player, {body} until end of turn."))
}
