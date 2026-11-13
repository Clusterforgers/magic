//! Wordings of the rmt4 card pack: an enters trigger of a Bargain card under "if it was bargained", and a token for
//! each opponent that's tapped and attacking that player.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Condition;

/// "When ~ enters, if it was bargained, ...": the optional additional cost of the permanent's own spell
/// (`Condition::AdditionalCostPaid`), which the writer words as the gift.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    bargained(ability, texts);
    if matches!(ability, Ability::Static { .. }) {
        let more: Vec<String> = texts
            .iter()
            .flat_map(|text| anthem_wordings(text))
            .collect();
        for text in more {
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
    for text in each_opponent_attacked(ability) {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

fn bargained(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
        intervening_if: Some(Condition::AdditionalCostPaid),
        ..
    } = ability
    else {
        return;
    };
    if what.0.as_slice() != [Pred::This] {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            text.strip_prefix("When ~ enters, if the gift was promised, ")
                .map(|rest| format!("When ~ enters, if it was bargained, {rest}"))
        })
        .collect();
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "For each opponent, create a 1/1 black Ninja creature token that's tapped and attacking that player.": a triggered
/// ability whose one effect repeats over the opponents a token that enters tapped and attacking the opponent of the pass.
/// It is worded from the same ability with the token attacking no one named.
fn each_opponent_attacked(ability: &Ability) -> Vec<String> {
    let Ability::Triggered { effects, .. } = ability else {
        return Vec::new();
    };
    let [
        Effect::RepeatEach {
            over: Ref::Opponents,
            bind,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let [
        Effect::CreateToken {
            tapped: true,
            attacking: true,
            against: Some(against),
            blocking: None,
            bind: None,
            ..
        },
    ] = inner.as_slice()
    else {
        return Vec::new();
    };
    // "that player", or "that player or a planeswalker they control" (r40-7: the controller chooses among them).
    let walkers = if *against == Ref::Bound(bind.clone()) {
        false
    } else if *against == r407_player_or_walkers(bind) {
        true
    } else {
        return Vec::new();
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { effects, .. } = &mut plain {
        let mut token = inner.clone();
        if let Some(Effect::CreateToken { against, .. }) = token.first_mut() {
            *against = None;
        }
        *effects = token;
    }
    let mut found = Vec::new();
    for text in super::render::written_variants(&plain) {
        let Some((head, rest)) = text.split_once(", create ") else {
            continue;
        };
        let Some(token) = rest.strip_suffix(" that's tapped and attacking.") else {
            continue;
        };
        for (subject, player) in [("", "player"), ("you ", "opponent")] {
            let or = if walkers {
                " or a planeswalker they control"
            } else {
                ""
            };
            found.push(format!(
                "{head}, for each opponent, {subject}create {token} that's tapped and attacking that {player}{or}."
            ));
        }
    }
    found
}

/// The defenders "that player or a planeswalker they control" for the player bound as `bind`.
fn r407_player_or_walkers(bind: &str) -> Ref {
    Ref::Union(vec![
        Ref::Bound(bind.to_owned()),
        Ref::Each(crate::refs::Selector(vec![
            Pred::Type(mtg_core::card::CardType::Planeswalker),
            Pred::ControlledBy(Ref::Bound(bind.to_owned())),
        ])),
    ])
}

/// The other wordings of a static ability over creatures of several subtypes: the articles of a list after "that's"
/// left out but the first ("that's a Cat, Elemental, or Beast"), and "Each other creature you control that's a Ninja or
/// a Rogue gets" as "Other Ninja and Rogue creatures you control get".
fn anthem_wordings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(bare) = bare_list(text) {
        found.push(bare);
    }
    if let Some(plural) = plural_group(text) {
        found.push(plural);
    }
    found
}

/// "that's a Cat, an Elemental, or a Beast" as "that's a Cat, Elemental, or Beast".
fn bare_list(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("that's ")?;
    let (list, tail) = rest.split_once(" gets ").map_or_else(
        || {
            rest.split_once(" get ")
                .map(|(l, t)| (l, format!(" get {t}")))
        },
        |(l, t)| Some((l, format!(" gets {t}"))),
    )?;
    let items = subtype_items(list)?;
    if items.len() < 2 {
        return None;
    }
    let (first_article, first) = items.first()?;
    let names: Vec<&str> = items.iter().skip(1).map(|(_, name)| *name).collect();
    let (final_name, between) = names.split_last()?;
    let comma = if items.len() > 2 { "," } else { "" };
    let mut listed = vec![format!("{first_article} {first}")];
    listed.extend(between.iter().map(|name| (*name).to_owned()));
    let joined = format!("{}{comma} or {final_name}", listed.join(", "));
    Some(format!("{head}that's {joined}{tail}"))
}

/// The items of "a Cat, an Elemental, or a Beast" (or "a Ninja or a Rogue"): each article and capitalized name.
fn subtype_items(list: &str) -> Option<Vec<(&str, &str)>> {
    let mut items = Vec::new();
    for part in list.split(", ") {
        let part = part.strip_prefix("or ").unwrap_or(part);
        for piece in part.split(" or ") {
            let (article, name) = piece.split_once(' ')?;
            let capital = name.chars().next().is_some_and(char::is_uppercase);
            if !matches!(article, "a" | "an") || !capital || name.contains(' ') {
                return None;
            }
            items.push((article, name));
        }
    }
    Some(items)
}

/// "Each other creature you control that's a Ninja or a Rogue gets +1/+1." as "Other Ninja and Rogue creatures you
/// control get +1/+1.": two subtypes, and "has" after it as "have".
fn plural_group(text: &str) -> Option<String> {
    let rest = text.strip_prefix("Each other creature you control that's ")?;
    let (list, tail) = rest.split_once(" gets ")?;
    let items = subtype_items(list)?;
    let [(_, first), (_, second)] = items.as_slice() else {
        return None;
    };
    let tail = tail.replace(" and has ", " and have ");
    Some(format!(
        "Other {first} and {second} creatures you control get {tail}"
    ))
}
