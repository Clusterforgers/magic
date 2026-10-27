//! Tokens counted by what a player controls or by the creatures attacking: "Create X 1/2 green Spider creature tokens
//! with reach, where X is the number of creatures attacking you." and "Choose target opponent. You create X 3/3 green
//! Beast creature tokens, where X is the number of artifacts that player controls." The renderers word the attackers
//! "attacking creatures" and the controller of a counted group as the target.

use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Targetable};
use crate::value::Amount;

/// Adds the wordings of the tokens counted this way.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let Some(Effect::CreateToken {
        who: Ref::You,
        count: Amount::Count(counted),
        ..
    }) = effects.first()
    else {
        return;
    };
    if counted.0.contains(&Pred::AttackingPlayer(Ref::You)) {
        attackers(ability, texts);
    } else if matches!(
        targets.as_slice(),
        [spec] if matches!(spec.what, Targetable::Opponent)
    ) && counted.0.contains(&Pred::ControlledBy(Ref::Target(0)))
    {
        opponents(ability, texts);
    }
}

/// "the number of attacking creatures" names the player attacked in the oracle text.
fn attackers(ability: &Ability, texts: &mut Vec<String>) {
    let mut plain = ability.clone();
    if let Ability::Spell { effects, .. } = &mut plain
        && let Some(Effect::CreateToken {
            count: Amount::Count(counted),
            ..
        }) = effects.first_mut()
    {
        for pred in &mut counted.0 {
            if *pred == Pred::AttackingPlayer(Ref::You) {
                *pred = Pred::Attacking;
            }
        }
    }
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter(|text| text.contains(", where X is the number of attacking creatures"))
        .map(|text| {
            text.replacen(
                ", where X is the number of attacking creatures",
                ", where X is the number of creatures attacking you",
                1,
            )
        })
        .collect();
    texts.extend(said);
}

/// The opponent is chosen in a sentence of its own and the counted group belongs to "that player".
fn opponents(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = variants(ability)
        .iter()
        .filter_map(|text| {
            let body = text.strip_prefix("Create X ")?;
            let (tokens, group) = body.split_once(", where X is the number of ")?;
            let group = group.strip_suffix(" target opponent controls.")?;
            Some(format!(
                "Choose target opponent. You create X {tokens}, where X is the number of {group} that player controls."
            ))
        })
        .collect();
    texts.extend(said);
}
