//! A shield for a while over the creatures that enter: "This turn, each creature you control enters with an additional
//! +1/+1 counter on it." and, beside a permission to cast creature spells with flash, "Until your next turn, you may cast
//! creature spells as though they had flash, and each creature you control enters with an additional +1/+1 counter on
//! it."

use super::super::amounts::{count_word, counted_parts};
use crate::compile::read::triggers::{pattern_text, subject_text};
use crate::effect::Effect;
use crate::modify::{CastPermission, Duration, GameRule};
use crate::refs::{Pred, Ref};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, CounterKind, StatKind};

/// "this turn" and "until your next turn": the durations the sentences name.
fn lasting(duration: &Duration) -> Option<&'static str> {
    match duration {
        Duration::UntilEndOfTurn => Some("this turn"),
        Duration::UntilYourNextTurn => Some("until your next turn"),
        _ => None,
    }
}

/// "an additional +1/+1 counter on it", "two additional +1/+1 counters on it".
pub(super) fn rmt3_additional(count: &Amount) -> Option<String> {
    match count {
        Amount::Fixed(1) => Some("an additional +1/+1 counter on it".to_owned()),
        Amount::Fixed(n) => {
            let word = count_word(u32::try_from(*n).ok()?)?;
            Some(format!("{word} additional +1/+1 counters on it"))
        }
        _ => None,
    }
}

/// "each creature you control enters with an additional +1/+1 counter on it" and its duration, for a shield of +1/+1
/// counters over a group that enters.
fn group_enters(effect: &Effect) -> Option<(String, &Duration)> {
    let Effect::Shield {
        when: ReplaceWhen::Enters { what },
        by:
            ReplaceBy::WithCounters {
                counter: CounterKind::PlusOnePlusOne,
                count,
                tapped: false,
            },
        duration,
    } = effect
    else {
        return None;
    };
    if what.0.contains(&Pred::This) {
        return None;
    }
    let group = subject_text(what)?;
    let group = group
        .strip_prefix("a ")
        .or_else(|| group.strip_prefix("an "))?;
    let counters = rmt3_additional(count)?;
    Some((format!("each {group} enters with {counters}"), duration))
}

/// "this turn, each creature you control enters with an additional +1/+1 counter on it": the shield alone.
pub(super) fn rmt3_shield_text(effect: &Effect) -> Option<String> {
    let (text, duration) = group_enters(effect)?;
    Some(format!("{}, {text}", lasting(duration)?))
}

/// The permission to cast creature spells with flash for the shield's duration, then the shield, in one sentence.
pub(super) fn rmt3_flash_and_shield(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Rule {
            rule: GameRule::MayCast(permission),
            duration: first,
        },
        shield,
        ..,
    ] = effects
    else {
        return None;
    };
    let (text, duration) = group_enters(shield)?;
    if first != duration || !creature_flash(permission) {
        return None;
    }
    Some((
        format!(
            "{}, you may cast creature spells as though they had flash, and {text}",
            lasting(duration)?
        ),
        2,
    ))
}

/// A plain permission of the controller to cast creature spells as though they had flash, and nothing more.
fn creature_flash(permission: &CastPermission) -> bool {
    serde_json::to_string(permission)
        .is_ok_and(|json| json == r#"{"what":[{"Type":"Creature"}],"flash":true}"#)
}

/// "that creature enters with an additional +1/+1 counter on it" and its counted forms ("... for each Dog you control",
/// "X additional +1/+1 counters on it, where X is its mana value minus 4"), for the spell that set off a cast trigger.
pub(super) fn rmt3_spell_enters_text(effect: &Effect) -> Option<String> {
    let Effect::SpellEntersWith {
        spell: Ref::Triggering,
        count,
    } = effect
    else {
        return None;
    };
    let gets = match count {
        Amount::Fixed(_) => rmt3_additional(count)?,
        Amount::Sum(parts) => match parts.as_slice() {
            [
                Amount::Stat {
                    of: Ref::Triggering,
                    stat: StatKind::ManaValue,
                },
                Amount::Fixed(less),
            ] if *less < 0 => format!(
                "X additional +1/+1 counters on it, where X is its mana value minus {}",
                less.unsigned_abs()
            ),
            _ => return None,
        },
        Amount::Counters {
            on: Ref::This,
            counter,
        } => format!(
            "X additional +1/+1 counters on it, where X is the number of {} counters on ~",
            super::super::counters::kind_text(counter)?
        ),
        counted => {
            let (Amount::Fixed(1), tail) = counted_parts(counted)? else {
                return None;
            };
            format!("an additional +1/+1 counter on it {tail}")
        }
    };
    Some(format!("that creature enters with {gets}"))
}

/// "when you next cast a creature spell this turn, that creature enters with an additional +1/+1 counter on it": a
/// delayed trigger of the turn on the controller's next cast whose one step is the counters of that spell.
pub(super) fn rmt3_next_cast_text(effect: &Effect) -> Option<String> {
    let Effect::DelayedTrigger {
        until: None,
        when: when @ TriggerPattern::SpellCast { by: Ref::You, .. },
        once: true,
        this_turn: true,
        targets: delayed_targets,
        effects,
    } = effect
    else {
        return None;
    };
    if !delayed_targets.is_empty() {
        return None;
    }
    let [enters] = effects.as_slice() else {
        return None;
    };
    let spell = pattern_text(when)?
        .strip_prefix("Whenever you cast ")?
        .to_owned();
    Some(format!(
        "when you next cast {spell} this turn, {}",
        rmt3_spell_enters_text(enters)?
    ))
}

/// The other printed wordings: counters moved "onto a second target creature" are also printed "onto another target
/// creature", a player's second sacrifice leaves out its verb, and "that creature enters with" after the next cast of a spell is also printed "it enters with".
pub(super) fn rmt3_variants(texts: &mut Vec<String>) {
    let another: Vec<String> = texts
        .iter()
        .filter(|text| {
            (text.contains("counters from target") || text.contains(" counter from target"))
                && text.contains(" onto a second target ")
        })
        .map(|text| text.replacen(" onto a second target ", " onto another target ", 1))
        .collect();
    texts.extend(another);
    // "Each opponent sacrifices an artifact creature and sacrifices a nonartifact creature of their choice." is printed
    // with the second verb left out.
    let elided: Vec<String> = texts
        .iter()
        .filter(|text| {
            text.matches(" sacrifices ").count() == 2 && text.contains(" and sacrifices ")
        })
        .map(|text| text.replacen(" and sacrifices ", " and ", 1))
        .collect();
    texts.extend(elided);
    let more: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" this turn, that creature enters with "))
        .map(|text| {
            text.replacen(
                " this turn, that creature enters with ",
                " this turn, it enters with ",
                1,
            )
        })
        .collect();
    texts.extend(more);
}

/// "you may cast creature spells this turn as though they had flash" and "until your next turn, you may cast sorcery
/// spells as though they had flash": a plain permission of one card type for a while.
pub(super) fn rmt3_flash_text(effect: &Effect) -> Option<String> {
    let Effect::Rule {
        rule: GameRule::MayCast(permission),
        duration,
    } = effect
    else {
        return None;
    };
    let json = serde_json::to_string(permission).ok()?;
    let kind = ["Creature", "Sorcery", "Instant", "Artifact", "Enchantment"]
        .into_iter()
        .find(|kind| json == format!(r#"{{"what":[{{"Type":"{kind}"}}],"flash":true}}"#))?
        .to_lowercase();
    match duration {
        Duration::UntilEndOfTurn => Some(format!(
            "you may cast {kind} spells this turn as though they had flash"
        )),
        Duration::UntilYourNextTurn => Some(format!(
            "until your next turn, you may cast {kind} spells as though they had flash"
        )),
        _ => None,
    }
}
