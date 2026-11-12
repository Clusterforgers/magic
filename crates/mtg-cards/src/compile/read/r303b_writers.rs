//! r303b writers: "You may reveal this card from your opening hand. If you do, at the beginning of the first upkeep, ..."
//! and the target "creature, planeswalker, or player".

use mtg_core::card::CardType;

use crate::ability::Ability;
use crate::refs::{Pred, Ref, Targetable};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::trigger::{Step, TriggerPattern};

/// The wordings of an opening-hand reveal whose delayed trigger waits for the first upkeep: the effects as a spell
/// says them, after the reveal.
pub(super) fn r303b_whole(ability: &Ability) -> Option<Vec<String>> {
    if let Some(text) = r303b_combat_prevented(ability) {
        return Some(vec![text]);
    }
    let Ability::OpeningReveal { when, effects } = ability else {
        return None;
    };
    let TriggerPattern::StepBegins {
        step: Step::Upkeep,
        whose: Ref::EachPlayer,
    } = when
    else {
        return None;
    };
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: effects.clone(),
    };
    let texts: Vec<String> = super::render::variants(&spell)
        .into_iter()
        .filter_map(|text| {
            let mut chars = text.chars();
            let first = chars.next()?;
            Some(format!(
                "You may reveal this card from your opening hand. If you do, at the beginning of the first upkeep, {}{}",
                first.to_lowercase(),
                chars.as_str()
            ))
        })
        .collect();
    (!texts.is_empty()).then_some(texts)
}

/// Whether the kinds are a creature, a planeswalker and a player, in the order "creature, planeswalker, or player".
pub(super) fn r303b_three_kinds(kinds: &[Targetable]) -> bool {
    let object = |kind: CardType| Targetable::Object(crate::refs::Selector(vec![Pred::Type(kind)]));
    kinds
        == [
            object(CardType::Creature),
            object(CardType::Planeswalker),
            Targetable::Player,
        ]
}

/// "Prevent all combat damage that would be dealt to ~ by Walls.": combat damage to the card from one subtype prevented.
fn r303b_combat_prevented(ability: &Ability) -> Option<String> {
    let Ability::Replacement {
        when:
            ReplaceWhen::Damage {
                to: Some(Ref::This),
                source,
                combat: Some(true),
            },
        by: ReplaceBy::Prevent,
        condition: None,
        ..
    } = ability
    else {
        return None;
    };
    let [Pred::Subtype(kind)] = source.0.as_slice() else {
        return None;
    };
    Some(format!(
        "Prevent all combat damage that would be dealt to ~ by {}s.",
        kind.0
    ))
}
