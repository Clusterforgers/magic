//! r40-0 wordings of whole abilities: attack requirements that follow another attacker ("If a creature you control
//! attacks, this creature also attacks if able.", "If a creature with a magnet counter on it attacks, all creatures with
//! magnet counters on them attack if able.").

use crate::ability::Ability;
use crate::compile::read::conditional::condition_texts;
use crate::compile::read::selectors::{Form, phrase_text};
use crate::compile::read::triggers::condition_text;
use crate::modify::{CombatQuirk, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::CounterKind;

/// Adds the wordings of the r40-0 whole abilities.
pub(super) fn r400_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    texts.extend(r400_follow(ability));
    texts.extend(r400_blocked_by_all(ability));
    texts.extend(r400_reveal_hand(ability));
}

/// "If <condition>, you may reveal your hand rather than pay this spell's mana cost."
fn r400_reveal_hand(ability: &Ability) -> Vec<String> {
    let Ability::FreeCast {
        condition: Some(condition),
        ..
    } = ability
    else {
        return Vec::new();
    };
    if super::super::super::r400_reads::r400_reveal_hand(Some(condition.clone())) != *ability {
        return Vec::new();
    }
    let mut said = condition_texts(condition);
    if let Some(one) = condition_text(condition)
        && !said.contains(&one)
    {
        said.push(one);
    }
    said.into_iter()
        .map(|held| format!("If {held}, you may reveal your hand rather than pay ~'s mana cost."))
        .collect()
}

/// "Whenever a <creature> attacks, all <creatures> block that creature this turn if able."
fn r400_blocked_by_all(ability: &Ability) -> Option<String> {
    let value = serde_json::to_value(ability).ok()?;
    let subject: Selector =
        serde_json::from_value(value.pointer("/Triggered/when/Attacks/what")?.clone()).ok()?;
    let all: Selector = serde_json::from_value(
        value
            .pointer("/Triggered/effects/0/Rule/rule/MustBlockAttacker/who/Each")?
            .clone(),
    )
    .ok()?;
    if super::super::super::r400_reads::r400_blocked_by_all(&subject, &all).as_ref()
        != Some(ability)
    {
        return None;
    }
    let one = phrase_text(&subject, Form::Subject)?;
    let all = r400_all_text(&all)?;
    Some(format!(
        "Whenever {} {one} attacks, all {all} block that creature this turn if able.",
        r400_article(&one)
    ))
}

/// The words of "all <creatures>", the counters on them said after the creatures ("creatures with magnet counters on
/// them").
fn r400_all_text(all: &Selector) -> Option<String> {
    if let Some(text) = phrase_text(all, Form::All) {
        return Some(text);
    }
    let (last, rest) = all.0.split_last()?;
    let Pred::HasCounter(CounterKind::Other(kind)) = last else {
        return None;
    };
    let rest = phrase_text(&Selector(rest.to_vec()), Form::All)?;
    Some(format!("{rest} with {kind} counters on them"))
}

/// "a" or "an" before `noun`.
fn r400_article(noun: &str) -> &'static str {
    if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

/// The words of an attack requirement that follows another attacker (`CombatQuirk::AttackIfAnyAttacks`): of the card
/// itself, or of every creature the selector picks when it binds those same creatures.
fn r400_follow(ability: &Ability) -> Option<String> {
    let Ability::Rule {
        rule:
            GameRule::Combat {
                who,
                quirk: CombatQuirk::AttackIfAnyAttacks(among),
            },
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return None;
    };
    let one = phrase_text(among, Form::Subject)?;
    let head = format!("If {} {one} attacks", r400_article(&one));
    match who {
        Ref::This => Some(format!("{head}, ~ also attacks if able.")),
        Ref::Each(bound) if bound == among => {
            let all = r400_all_text(bound)?;
            Some(format!("{head}, all {all} attack if able."))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an attack requirement that follows another attacker is written as the cards print it, and one whose bound
    //! creatures differ from the creatures it follows is not written.
    use super::*;
    use mtg_core::card::CardType;
    use rstest::rstest;

    fn r400_rule(who: Ref, among: Vec<Pred>) -> Ability {
        Ability::Rule {
            rule: GameRule::Combat {
                who,
                quirk: CombatQuirk::AttackIfAnyAttacks(Selector(among)),
            },
            condition: None,
            from: ZoneKind::Battlefield,
        }
    }

    fn r400_magnet() -> Vec<Pred> {
        vec![
            Pred::Type(CardType::Creature),
            Pred::HasCounter(CounterKind::Other("magnet".to_owned())),
        ]
    }

    #[rstest]
    #[case::itself(
        r400_rule(Ref::This, vec![Pred::Type(CardType::Creature), Pred::ControlledBy(Ref::You)]),
        Some("If a creature you control attacks, ~ also attacks if able.")
    )]
    #[case::all_of_a_kind(
        r400_rule(Ref::Each(Selector(r400_magnet())), r400_magnet()),
        Some(
            "If a creature with a magnet counter on it attacks, all creatures with magnet counters on them attack if able."
        )
    )]
    #[case::other_creatures_bound(
        r400_rule(Ref::Each(Selector(vec![Pred::Type(CardType::Creature)])), r400_magnet()),
        None
    )]
    fn r400_follow_is_written_as_printed(#[case] ability: Ability, #[case] text: Option<&str>) {
        assert_eq!(r400_follow(&ability).as_deref(), text);
    }
}
