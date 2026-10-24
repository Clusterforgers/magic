//! A target chosen in a sentence of its own before counters go on it: "choose target creature attacking you. Put a stun
//! counter on that creature." The renderers put the counters on the target in one sentence, and an attacker of the
//! controller is "attacking" without the words "you". An opponent's attack names the attacked player without "with one or
//! more creatures".

use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Targetable};
use crate::trigger::TriggerPattern;

/// Adds the wordings of the choice of the one target and the counters put on it.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::PlayerAttacks { .. },
        targets,
        effects,
        ..
    } = ability
    else {
        return;
    };
    if targets.len() != 1
        || !matches!(
            effects.as_slice(),
            [Effect::PutCounters {
                on: Ref::Target(0),
                ..
            }]
        )
    {
        return;
    }
    // The writers of a selector word an attacker as "attacking creature"; the oracle names the player attacked.
    let mut plain = ability.clone();
    let mut at_you = false;
    if let Ability::Triggered { targets, .. } = &mut plain {
        for pred in targets.iter_mut().flat_map(|spec| match &mut spec.what {
            Targetable::Object(selector) => selector.0.iter_mut(),
            _ => [].iter_mut(),
        }) {
            if *pred == Pred::AttackingPlayer(Ref::You) {
                *pred = Pred::Attacking;
                at_you = true;
            }
        }
    }
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| chosen(text, at_you))
        .collect();
    texts.extend(said);
}

/// "..., put A on target N." as "..., choose target N. Put A on that N.", with "attacking N" as "N attacking you".
fn chosen(text: &str, at_you: bool) -> Option<String> {
    let text = text.replacen(" with one or more creatures", "", 1);
    let (head, rest) = text.split_once(", put ")?;
    let (counters, target) = rest.split_once(" on target ")?;
    let target = target.strip_suffix('.')?;
    let target = if at_you {
        format!("{} attacking you", target.strip_prefix("attacking ")?)
    } else {
        target.to_owned()
    };
    let noun = target.split(' ').next()?;
    Some(format!(
        "{head}, choose target {target}. Put {counters} on that {noun}."
    ))
}
