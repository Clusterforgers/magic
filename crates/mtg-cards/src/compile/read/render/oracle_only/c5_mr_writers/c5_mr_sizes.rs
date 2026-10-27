//! Sizes and counts of the fifth sample of M to R: "double target creature's power and toughness until end of turn",
//! "+1/+1 for each card in its controller's hand", "+1/+1 for each land you control and each land card in your
//! graveyard", "If a Dinosaur is dealt damage this way, ~ gets +2/+0 until end of turn.", "to each of X targets" and
//! "up to X target ..., where X is the number of times ~ was kicked" and "then it deals X damage to any target, where X
//! is the number of ...".

use super::{c5_mr_swapped, c5_mr_twin, c5_mr_twin_all};
use crate::ability::Ability;
use crate::modify::Modification;
use crate::value::Amount;

/// The size of the target doubled, and the +1/+1 the renderers write instead.
const C5_MR_DOUBLE: &str = r#"{"ModifyPT":{"power":{"Stat":{"of":{"Target":0},"stat":"Power"}},"toughness":{"Stat":{"of":{"Target":0},"stat":"Toughness"}}}}"#;
const C5_MR_PLUS_ONE: &str = r#"{"ModifyPT":{"power":1,"toughness":1}}"#;

/// The hand of the controller of the enchanted creature, and your hand the renderers write.
const C5_MR_THEIR_HAND: &str = r#"{"OwnedBy":{"ControllerOf":"Attached"}}"#;
const C5_MR_YOUR_HAND: &str = r#"{"OwnedBy":"You"}"#;

/// The pump of the source when the creature of the event has a subtype, after damage to that creature.
const C5_MR_DEALT_HEAD: &str = r#"{"DealDamage":{"to":"Triggering","amount":"#;
const C5_MR_DEALT_IF: &str =
    r#"{"If":{"condition":{"Matches":{"subject":"Triggering","selector":[{"Subtype":""#;
const C5_MR_DEALT_PUMP: &str =
    r#""}]}},"then":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":"#;

/// Exactly X targets, and the single target the renderers write.
const C5_MR_X_TARGETS: &str = r#","bounds":{"min":"X","max":"X"}}],"effects":[{"DealDamage""#;
const C5_MR_ONE_TARGET: &str = r#"}],"effects":[{"DealDamage""#;

/// Up to as many targets as the spell was kicked, and up to two the renderers write.
const C5_MR_KICKS: &str = r#""bounds":{"min":0,"max":"Kicks"}"#;
const C5_MR_TWO: &str = r#""bounds":{"min":0,"max":2}"#;

/// Adds the wordings of the sizes and counts.
pub(super) fn add(ability: &Ability, json: &str, texts: &mut Vec<String>) {
    let twin = c5_mr_twin(json, C5_MR_DOUBLE, C5_MR_PLUS_ONE);
    for from in [
        "target creature gets +1/+1 until end of turn",
        "until end of turn, target creature gets +1/+1",
    ] {
        texts.extend(c5_mr_swapped(
            &twin,
            from,
            "double target creature's power and toughness until end of turn",
        ));
    }
    let twin = c5_mr_twin_all(json, &[(C5_MR_THEIR_HAND, C5_MR_YOUR_HAND); 2]);
    for (from, to) in [
        (
            " for each card in your hand",
            " for each card in its controller's hand",
        ),
        (" cards in your hand", " cards in its controller's hand"),
    ] {
        texts.extend(c5_mr_swapped(&twin, from, to));
    }
    c5_mr_each_sum(ability, texts);
    if json.contains(r#"{"DealDamage":{"to":{"Target":0},"amount":{"Count":"#) {
        let found: Vec<String> = texts
            .iter()
            .filter_map(|text| c5_mr_then_it_deals(text))
            .collect();
        texts.extend(found);
    }
    c5_mr_dealt_this_way(json, texts);
    let twin = c5_mr_twin(json, C5_MR_X_TARGETS, C5_MR_ONE_TARGET);
    texts.extend(c5_mr_swapped(
        &twin,
        " to any target.",
        " to each of X targets.",
    ));
    let found: Vec<String> = c5_mr_twin(json, C5_MR_KICKS, C5_MR_TWO)
        .iter()
        .filter(|text| text.contains("up to two target ") && text.ends_with('.'))
        .map(|text| {
            let text = text.replacen("up to two target ", "up to X target ", 1);
            let head = text.strip_suffix('.').unwrap_or(&text);
            format!("{head}, where X is the number of times ~ was kicked.")
        })
        .collect();
    texts.extend(found);
}

/// "create ..., then it deals X damage to any target, where X is the number of ..." for "create .... Then ~ deals damage
/// to any target equal to the number of ...", with "you create" opening the effects written "create".
fn c5_mr_then_it_deals(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(". Then ~ deals damage to ")?;
    let (target, count) = rest.split_once(" equal to the number of ")?;
    let head = head.replacen(", you create ", ", create ", 1);
    Some(format!(
        "{head}, then it deals X damage to {target}, where X is the number of {count}"
    ))
}

/// "+1/+1 for each A and each B" for a size that is the sum of two counts, from the twins with each count alone.
fn c5_mr_each_sum(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static { mods, .. } = ability else {
        return;
    };
    let [Modification::ModifyPT { power, toughness }] = mods.as_slice() else {
        return;
    };
    let Amount::Sum(parts) = power else {
        return;
    };
    let [first, second] = parts.as_slice() else {
        return;
    };
    if power != toughness {
        return;
    }
    let alone = |part: &Amount| {
        let mut twin = ability.clone();
        if let Ability::Static { mods, .. } = &mut twin {
            *mods = vec![Modification::ModifyPT {
                power: part.clone(),
                toughness: part.clone(),
            }];
        }
        super::super::super::variants(&twin)
    };
    let seconds = alone(second);
    let mut found = Vec::new();
    for text in alone(first) {
        let Some((head, kind)) = text.split_once(" for each ") else {
            continue;
        };
        for other in &seconds {
            if let Some(more) = other
                .strip_prefix(head)
                .and_then(|rest| rest.strip_prefix(" for each "))
            {
                let kind = kind.strip_suffix('.').unwrap_or(kind);
                found.push(format!("{head} for each {kind} and each {more}"));
            }
        }
    }
    texts.extend(found);
}

/// "~ deals 2 damage to it. If a Dinosaur is dealt damage this way, ~ gets +2/+0 until end of turn." from the twin
/// without the pump.
fn c5_mr_dealt_this_way(json: &str, texts: &mut Vec<String>) {
    let Some(at) = json.find(C5_MR_DEALT_HEAD) else {
        return;
    };
    let Some(rest) = json.get(at..) else {
        return;
    };
    let Some(start) = rest.find(C5_MR_DEALT_IF) else {
        return;
    };
    let Some(after) = rest.get(start + C5_MR_DEALT_IF.len()..) else {
        return;
    };
    let Some((kind, tail)) = after.split_once(C5_MR_DEALT_PUMP) else {
        return;
    };
    let Some((power, tail)) = tail.split_once(r#","toughness":"#) else {
        return;
    };
    let Some((toughness, close)) = tail.split_once('}') else {
        return;
    };
    let pump_end = r#"}],"duration":"UntilEndOfTurn"}}]}}"#;
    if !close.starts_with(pump_end) || kind.contains('"') {
        return;
    }
    let (Ok(power), Ok(toughness)) = (power.parse::<i32>(), toughness.parse::<i32>()) else {
        return;
    };
    let whole = format!(
        "{C5_MR_DEALT_IF}{kind}{C5_MR_DEALT_PUMP}{power},\"toughness\":{toughness}}}{pump_end}"
    );
    let article = if kind.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    let pump = format!(
        " If {article} {kind} is dealt damage this way, ~ gets {power:+}/{toughness:+} until end of turn."
    );
    let found: Vec<String> = c5_mr_twin(json, &format!(",{whole}"), "")
        .iter()
        .map(|text| format!("{text}{pump}"))
        .collect();
    texts.extend(found);
}
