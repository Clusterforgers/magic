//! Triggers that untap creatures and add a combat phase, in the words of the cards: Lightning Runner ("you get {E}{E},
//! then you may pay eight {E}. If you pay, untap all creatures you control, and after this phase, there is an
//! additional combat phase."), Godo ("untap it and all Samurai you control. After this phase, ...") and Scourge of the
//! Throne ("..., if it's attacking the player with the most life or tied for most life, untap all attacking
//! creatures.").

use super::super::variants;
use crate::ability::Ability;
use crate::effect::{Effect, PhaseKind};
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition};

/// Whether the ability adds one combat phase after an untap.
fn untaps_and_adds(effects: &[Effect]) -> bool {
    let adds = |effects: &[Effect]| matches!(effects.last(), Some(Effect::AddPhase { phases }) if phases == &[PhaseKind::Combat]);
    effects.iter().any(|effect| match effect {
        Effect::Pay { paid, .. } => adds(paid),
        _ => false,
    }) || adds(effects)
}

/// The energy of the trigger is a sentence of its own with "then" and the payment says "if you pay".
fn energy_then_pay(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains("{E} and you may pay "))
        .map(|text| {
            text.replace("{E} and you may pay ", "{E}, then you may pay ")
                .replace(". If you do, untap ", ". If you pay, untap ")
                .replace(
                    " you control and after this phase",
                    " you control, and after this phase",
                )
        })
        .collect()
}

/// "untap it and all Samurai you control" for the source and the group, with the added phase as the next sentence.
fn untap_it_and_all(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let (head, tail) = text.split_once("untap ~. Untap all ")?;
            let (group, rest) = tail.split_once(" you control. Then after this phase")?;
            Some(format!(
                "{head}untap it and all {group} you control. After this phase{rest}"
            ))
        })
        .collect()
}

/// An intervening "if" that the defending player has the most life, written ahead of the effects.
fn most_life(ability: &Ability) -> Option<Vec<String>> {
    let Ability::Triggered {
        intervening_if:
            Some(Condition::Compare {
                left: Amount::Life(Ref::Defending),
                op: Cmp::Ge,
                right: Amount::HighestLife(Ref::EachPlayer),
            }),
        ..
    } = ability
    else {
        return None;
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let said = variants(&plain)
        .into_iter()
        .filter_map(|text| {
            let (head, tail) = text.split_once(", ")?;
            Some(format!(
                "{head}, if it's attacking the player with the most life or tied for most life, {tail}"
            ))
        })
        .collect();
    Some(said)
}

/// "untap each attacking creature" for the group, the added phase as a sentence of its own, and the phase said after the
/// anchor when a condition stands before it ("If it's the first combat phase of your turn, there is an additional
/// combat phase after this phase.").
fn each_attacking(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains("an additional combat phase"))
        .filter_map(|text| {
            let said = text
                .replace("untap all attacking creatures", "untap each attacking creature")
                .replace(". Then after this phase,", ". After this phase,")
                .replace(
                    "If it's the first combat phase of your turn, after this phase, there is an additional combat phase.",
                    "If it's the first combat phase of your turn, there is an additional combat phase after this phase.",
                );
            (said != *text).then_some(said)
        })
        .collect()
}

/// Adds the wordings of the triggers that untap and add a combat phase.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    if let Some(said) = most_life(ability) {
        texts.extend(said);
    }
    let more = each_attacking(texts);
    texts.extend(more);
    if untaps_and_adds(effects) {
        let more = energy_then_pay(texts);
        texts.extend(more);
        let more = untap_it_and_all(texts);
        texts.extend(more);
    }
}
