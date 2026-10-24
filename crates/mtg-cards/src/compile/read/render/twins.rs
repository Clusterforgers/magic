//! Wordings the oracle text uses for an event or a subject that the data states with one more predicate than the plain
//! wording has: the ability is written without it and the words are added where the oracle puts them ("enters from a
//! graveyard", "enters untapped", "casts a spell from a graveyard", "this creature and land creatures you control",
//! "Each planeswalker you control enters with an additional loyalty counter on it", "..., if a creature died this turn,
//! you draw a card. If seven or more creatures died this turn, instead you draw seven cards."), and the nouns the oracle
//! says as one word ("outlaws").

use super::variants;
use crate::ability::Ability;
use crate::compile::read::triggers::subject_text;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, Condition};

/// A wording of the plain ability, and the same wording with the words of the predicate it leaves out.
type Edit = Box<dyn Fn(&str) -> Option<String>>;

/// The ability without the extra predicate, and the edit that puts its words back into a wording of the plain ability.
type Twin = (Ability, Edit);

/// Nouns the data words as an adjective with a noun and the oracle as one plural word, one line each.
const NOUNS: [(&str, &str); 1] = [("outlaw permanents", "outlaws")];

/// Adds to `texts` the wordings of the ability that its plain event or subject leaves out.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let twins = [
        entered_from_a_graveyard(ability),
        entered_untapped(ability),
        cast_from_a_graveyard(ability),
        itself_and_a_group(ability),
        group_enters_with_a_counter(ability),
    ];
    let mut found = instead_when_more(ability);
    for (plain, edit) in twins.into_iter().flatten() {
        found.extend(variants(&plain).iter().filter_map(|text| edit(text)));
    }
    texts.extend(found);
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (noun, word) = NOUNS.iter().find(|(noun, _)| text.contains(noun))?;
            Some(text.replace(noun, word))
        })
        .collect();
    texts.extend(said);
}

/// The ability with other effects and another intervening "if".
fn with_effects(ability: &Ability, condition: Option<Condition>, steps: &[Effect]) -> Ability {
    let mut twin = ability.clone();
    if let Ability::Triggered {
        intervening_if,
        effects,
        ..
    } = &mut twin
    {
        if condition.is_some() {
            *intervening_if = condition;
        }
        *effects = steps.to_vec();
    }
    twin
}

/// "..., if a creature died this turn, you draw a card. If seven or more creatures died this turn, instead you draw
/// seven cards.": a trigger with an intervening "if" whose one step does a bigger thing on a second count.
fn instead_when_more(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        intervening_if: Some(_),
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::If {
            condition,
            then,
            otherwise,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if then.is_empty() || otherwise.is_empty() {
        return Vec::new();
    }
    let more = variants(&with_effects(ability, Some(condition.clone()), then));
    let mut found = Vec::new();
    for usual in variants(&with_effects(ability, None, otherwise)) {
        for bigger in &more {
            if let Some((condition, step)) = bigger
                .split_once(", if ")
                .and_then(|(_, rest)| rest.split_once(", "))
            {
                found.push(format!("{usual} If {condition}, instead {step}"));
            }
        }
    }
    found
}

/// The ability with another event.
fn on_event(ability: &Ability, event: TriggerPattern) -> Ability {
    let mut twin = ability.clone();
    if let Ability::Triggered { when, .. } = &mut twin {
        *when = event;
    }
    twin
}

/// The event without the zone its card came from, when every event it lists is a card entering from a graveyard.
fn without_graveyard(event: &TriggerPattern) -> Option<TriggerPattern> {
    match event {
        TriggerPattern::ZoneChange {
            what,
            from: Some(ZoneKind::Graveyard),
            to: Some(ZoneKind::Battlefield),
        } => Some(TriggerPattern::ZoneChange {
            what: what.clone(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        }),
        TriggerPattern::AnyOf(events) => events
            .iter()
            .map(without_graveyard)
            .collect::<Option<Vec<_>>>()
            .map(TriggerPattern::AnyOf),
        _ => None,
    }
}

/// "When this creature enters from a graveyard, ...".
fn entered_from_a_graveyard(ability: &Ability) -> Option<Twin> {
    let Ability::Triggered { when, .. } = ability else {
        return None;
    };
    Some((
        on_event(ability, without_graveyard(when)?),
        Box::new(|text| {
            text.contains(" enters,")
                .then(|| text.replacen(" enters,", " enters from a graveyard,", 1))
        }),
    ))
}

/// "When this land enters untapped, ...": the land's own entry with the intervening "if" that it is untapped.
fn entered_untapped(ability: &Ability) -> Option<Twin> {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
        intervening_if:
            Some(Condition::Matches {
                subject: Ref::This,
                selector,
            }),
        ..
    } = ability
    else {
        return None;
    };
    if what.0 != [Pred::This] || selector.0 != [Pred::Untapped] {
        return None;
    }
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    Some((
        plain,
        Box::new(|text| {
            text.contains(" enters,")
                .then(|| text.replacen(" enters,", " enters untapped,", 1))
        }),
    ))
}

/// "Whenever a player casts a spell from a graveyard, ...".
fn cast_from_a_graveyard(ability: &Ability) -> Option<Twin> {
    let Ability::Triggered {
        when: TriggerPattern::SpellCast { what, by },
        ..
    } = ability
    else {
        return None;
    };
    let from = Pred::CastFrom(ZoneKind::Graveyard);
    if !what.0.contains(&from) {
        return None;
    }
    let rest = Selector(
        what.0
            .iter()
            .filter(|pred| **pred != from)
            .cloned()
            .collect(),
    );
    let event = TriggerPattern::SpellCast {
        what: rest,
        by: by.clone(),
    };
    Some((
        on_event(ability, event),
        Box::new(|text| {
            let at = text.find(',')?;
            Some(format!("{} from a graveyard{}", &text[..at], &text[at..]))
        }),
    ))
}

/// "this creature and land creatures you control get +2/+2": a group that includes the source, which the plain
/// wording of the other members leaves out.
fn itself_and_a_group(ability: &Ability) -> Option<Twin> {
    let Ability::Static {
        affected: Ref::Each(group),
        ..
    } = ability
    else {
        return None;
    };
    let (at, others) = group
        .0
        .iter()
        .enumerate()
        .find_map(|(at, pred)| match pred {
            Pred::AnyOf(preds) if preds.len() == 2 && preds[0] == Pred::This => {
                Some((at, preds[1].clone()))
            }
            _ => None,
        })?;
    let mut preds = group.0.clone();
    preds[at] = others;
    let mut plain = ability.clone();
    if let Ability::Static { affected, .. } = &mut plain {
        *affected = Ref::Each(Selector(preds));
    }
    Some((
        plain,
        Box::new(|text| {
            let start = if text.starts_with("As long as ") {
                text.find(", ")? + 2
            } else {
                0
            };
            let (before, group) = text.split_at(start);
            let mut letters = group.chars();
            let first = letters.next()?;
            Some(format!(
                "{before}this creature and {}{}",
                first.to_ascii_lowercase(),
                letters.as_str()
            ))
        }),
    ))
}

/// "Each planeswalker you control enters with an additional loyalty counter on it.": a group entering with one more
/// counter, worded from the entry of the source itself.
fn group_enters_with_a_counter(ability: &Ability) -> Option<Twin> {
    let Ability::Replacement {
        when: ReplaceWhen::Enters { what },
        by:
            ReplaceBy::WithCounters {
                count: Amount::Fixed(1),
                tapped: false,
                ..
            },
        condition: None,
        ..
    } = ability
    else {
        return None;
    };
    if what.0.contains(&Pred::This) {
        return None;
    }
    let group = subject_text(what)?;
    let group = group
        .strip_prefix("a ")
        .or_else(|| group.strip_prefix("an "))?
        .to_owned();
    let mut plain = ability.clone();
    if let Ability::Replacement { when, .. } = &mut plain {
        *when = ReplaceWhen::Enters {
            what: Selector(vec![Pred::This]),
        };
    }
    Some((
        plain,
        Box::new(move |text| {
            let counter = text
                .strip_prefix("~ enters with a ")
                .or_else(|| text.strip_prefix("~ enters with an "))?;
            Some(format!("Each {group} enters with an additional {counter}"))
        }),
    ))
}
