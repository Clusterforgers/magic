//! Two abilities the card prints as one sentence, each written by its own writer: "~ attacks or blocks each combat if
//! able.", "Enchanted creature can't be the target of spells and can't be enchanted by other Auras." and "Spells you cast
//! with mana value 5 or greater cost {1} less to cast and can't be countered."

use super::render::written_variants;
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::Ref;
use serde_json::Value;

/// Every wording of the two abilities as one sentence; empty for any other two.
pub(super) fn texts(first: &Ability, second: &Ability) -> Vec<String> {
    let mut texts = attacks_or_blocks(first, second);
    texts.extend(cant_both(first, second));
    texts.extend(cheaper_and_uncounterable(first, second));
    texts
}

/// The creature that must attack and the same creature that must block: "attacks or blocks each combat if able".
fn attacks_or_blocks(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Rule {
            rule: GameRule::MustAttack(who),
            condition: None,
            ..
        },
        Ability::Rule {
            rule: GameRule::MustBlock(other),
            condition: None,
            ..
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    if who != other {
        return Vec::new();
    }
    let blocks = written_variants(second);
    written_variants(first)
        .iter()
        .filter_map(|text| {
            let subject = text.strip_suffix(" attacks each combat if able.")?;
            blocks
                .iter()
                .any(|b| b == &format!("{subject} blocks each combat if able."))
                .then(|| format!("{subject} attacks or blocks each combat if able."))
        })
        .collect()
}

/// Two prohibitions of the same attached permanent: "Enchanted creature can't X and can't Y."
fn cant_both(first: &Ability, second: &Ability) -> Vec<String> {
    let cant = |ability: &Ability| {
        matches!(
            ability,
            Ability::Rule {
                rule: GameRule::Cant {
                    who: Ref::Attached,
                    ..
                },
                condition: None,
                ..
            }
        )
    };
    if !cant(first) || !cant(second) {
        return Vec::new();
    }
    let backs = written_variants(second);
    let mut texts = Vec::new();
    for front in written_variants(first) {
        let Some((subject, rest)) = front.split_once(" can't ") else {
            continue;
        };
        for back in &backs {
            if let Some(tail) = back
                .strip_prefix(subject)
                .and_then(|r| r.strip_prefix(" can't "))
            {
                let head = rest.strip_suffix('.').unwrap_or(rest);
                texts.push(format!("{subject} can't {head} and can't {tail}"));
            }
        }
    }
    texts
}

/// One replacement of damage to a player and the same one of damage to the permanents that player controls: "If a source
/// would deal damage to an opponent or a permanent an opponent controls, ..." is the two events of one sentence.
pub(super) fn damage_to_player_or_permanent(first: &Ability, second: &Ability) -> Vec<String> {
    let (Ok(first), Ok(second)) = (serde_json::to_value(first), serde_json::to_value(second))
    else {
        return Vec::new();
    };
    let event = |value: &Value| {
        let body = value.get("Replacement")?.as_object()?;
        (body.len() == 2).then_some(())?;
        Some((
            body.get("by")?.clone(),
            body.get("when")?.get("Damage")?.clone(),
        ))
    };
    let (Some((by, to)), Some((other_by, group))) = (event(&first), event(&second)) else {
        return Vec::new();
    };
    if by != other_by {
        return Vec::new();
    }
    let sentence = |who: &str, subject: &str, tail: &str| {
        let owned = serde_json::json!({ "to": { "Each": [{ "ControlledBy": who }] } });
        let ok = to == serde_json::json!({ "to": who }) && group == owned;
        ok.then(|| format!("If a source would deal damage to {subject}, {tail}"))
            .into_iter()
            .collect::<Vec<_>>()
    };
    if by == serde_json::json!({ "Multiply": 2 }) {
        return sentence(
            "Opponents",
            "an opponent or a permanent an opponent controls",
            "that source deals double that damage to that player or permanent instead.",
        );
    }
    if by == serde_json::json!("PreventHalf") {
        return sentence(
            "You",
            "you or a permanent you control",
            "prevent half that damage, rounded up.",
        );
    }
    Vec::new()
}

/// A cost reduction and the same spells that can't be countered; the spells are "you cast" in both, as controlled by
/// the player in the first and as cast by the player in the second.
fn cheaper_and_uncounterable(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::CostModifier {
            applies_to,
            condition: None,
            ..
        },
        Ability::Rule {
            rule: GameRule::CantBeCountered(Ref::Each(group)),
            condition: None,
            ..
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    let (Ok(Value::Array(own)), Ok(Value::Array(theirs))) = (
        serde_json::to_value(applies_to),
        serde_json::to_value(group),
    ) else {
        return Vec::new();
    };
    let controlled: Value = serde_json::json!({"ControlledBy": "You"});
    let cast: Value = serde_json::json!({"WasCast": {"by": "You"}});
    let swapped: Vec<Value> = own
        .into_iter()
        .map(|p| if p == controlled { cast.clone() } else { p })
        .collect();
    if swapped != theirs {
        return Vec::new();
    }
    written_variants(first)
        .iter()
        .filter_map(|text| {
            text.strip_suffix(" less to cast.")
                .map(|head| format!("{head} less to cast and can't be countered."))
        })
        .collect()
}
