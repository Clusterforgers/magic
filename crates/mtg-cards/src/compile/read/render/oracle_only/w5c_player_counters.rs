//! Counters a player gets: "you get an experience counter", "that player gets two rad counters". The step stands as a
//! marker gain of life while the ability is written, and the marker's clause is replaced by the player's sentence, so
//! the steps around it ("if they don't have any rad counters", "Otherwise, proliferate") read as in the ability.

use super::super::super::amounts::count_word;
use super::super::super::counters::kind_text;
use super::super::super::triggers::article;
use super::super::written_variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// The life of the marker gain that stands for the step.
const W5C_MARK: i32 = 97;

/// Adds the wordings of an ability whose top-level step puts counters on a player.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let Some((at, sentence)) = effects
        .iter()
        .enumerate()
        .find_map(|(at, effect)| Some((at, w5c_player_sentence(ability, effect)?)))
    else {
        return;
    };
    let mut twin = ability.clone();
    let marker = Effect::GainLife {
        who: Ref::You,
        amount: Amount::Fixed(W5C_MARK),
    };
    match &mut twin {
        Ability::Spell { effects, .. }
        | Ability::Activated { effects, .. }
        | Ability::Triggered { effects, .. }
        | Ability::Loyalty { effects, .. } => {
            if let Some(slot) = effects.get_mut(at) {
                *slot = marker;
            }
        }
        _ => return,
    }
    let lower = format!("you gain {W5C_MARK} life");
    let capital = format!("You gain {W5C_MARK} life");
    for text in written_variants(&twin) {
        if text.contains(&lower) {
            texts.push(text.replacen(&lower, &sentence, 1));
        } else if text.contains(&capital) {
            texts.push(text.replacen(&capital, &super::w4d_parts::capitalized(&sentence), 1));
        }
    }
}

/// "you get an experience counter", "that player gets two rad counters": counters on you, or on the player a damage
/// trigger names.
fn w5c_player_sentence(ability: &Ability, effect: &Effect) -> Option<String> {
    let Effect::PutCounters { on, counter, count } = effect else {
        return None;
    };
    let subject = match on {
        Ref::You => "you get",
        Ref::Triggering if w5c_damaged_player(ability) => "that player gets",
        _ => return None,
    };
    let kind = kind_text(counter)?;
    let Amount::Fixed(number) = count else {
        return None;
    };
    let number = u32::try_from(*number).ok()?;
    if number == 1 {
        return Some(format!("{subject} {} {kind} counter", article(kind)));
    }
    Some(format!("{subject} {} {kind} counters", count_word(number)?))
}

/// Whether the trigger is damage dealt to a player, so that "that player" is the one dealt damage.
fn w5c_damaged_player(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::DealsDamage {
                to: Some(Ref::EachPlayer | Ref::Opponents),
                ..
            },
            ..
        }
    )
}
