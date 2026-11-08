//! Two abilities the card prints as one sentence about either side of a fight: "Whenever ~ blocks or becomes blocked by
//! a creature, ~ deals 1 damage to that creature." and "Whenever enchanted creature attacks or blocks, its controller
//! loses 3 life." are two triggers, one for each event, whose effects name the other creature of each event; "Prevent all
//! damage that would be dealt to and dealt by enchanted creature." is two prevention shields.

use super::render::written_variants;
use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::refs::Selector;
use crate::trigger::TriggerPattern;
use serde_json::{Value, json};

/// Every wording of two abilities that read as one sentence, with a capital and a period; empty for any other two.
pub fn texts(first: &Ability, second: &Ability) -> Vec<String> {
    let mut texts = fight_texts(first, second);
    texts.extend(fight_texts(second, first));
    texts.extend(shield_texts(first, second));
    texts
}

/// "Whenever ~ blocks or becomes blocked by a creature, X" and "Whenever S attacks or blocks, X": `named` is the trigger
/// whose event holds the other creature as its subject (`Triggering`), `dealt` the one that holds it as the object that
/// did it (`Dealer`), with the same effects otherwise.
fn fight_texts(named: &Ability, dealt: &Ability) -> Vec<String> {
    let (
        Ability::Triggered {
            when,
            intervening_if: None,
            limit: None,
            from,
            ..
        },
        Ability::Triggered {
            when: other,
            intervening_if: None,
            limit: None,
            from: other_from,
            ..
        },
    ) = (named, dealt)
    else {
        return Vec::new();
    };
    if from != other_from || !same_but_dealer(named, dealt) {
        return Vec::new();
    }
    match (when, other) {
        // "~ blocks a creature" holds that creature; "~ becomes blocked by a creature" holds the blocker as the dealer.
        (
            TriggerPattern::BecomesBlocked {
                what: blocked,
                by: this,
                once: false,
            },
            TriggerPattern::BecomesBlocked {
                what: this_again,
                by: blocker,
                once: false,
            },
        ) if *this == Selector::this() && *this_again == Selector::this() && blocked == blocker => {
            blocks_either(named, blocked)
        }
        // The attacker is the event's subject; the blocker of a block is its dealer.
        (
            TriggerPattern::Attacks { what },
            TriggerPattern::Blocks {
                what: blocking,
                blocking: against,
            },
        ) if what == blocking && against.0.is_empty() => written_variants(named)
            .into_iter()
            .filter_map(|text| {
                let at = text.find(" attacks, ")?;
                Some(format!(
                    "{} attacks or blocks, {}",
                    &text[..at],
                    &text[at + " attacks, ".len()..]
                ))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// "Whenever ~ blocks or becomes blocked by a non-Wall creature, X": X is written as after "Whenever a non-Wall creature
/// attacks,", where the event's creature is "that creature".
fn blocks_either(named: &Ability, kind: &Selector) -> Vec<String> {
    let Some(phrase) = phrase_text(kind, Form::Subject) else {
        return Vec::new();
    };
    let article = if phrase.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    let mut stand = named.clone();
    if let Ability::Triggered { when, .. } = &mut stand {
        *when = TriggerPattern::Attacks { what: kind.clone() };
    }
    let head = format!("Whenever ~ blocks or becomes blocked by {article} {phrase}, ");
    let mut texts = Vec::new();
    for text in written_variants(&stand) {
        let Some(at) = text.find(" attacks, ").filter(|_| text.starts_with("Whenever ")) else {
            continue;
        };
        let body = &text[at + " attacks, ".len()..];
        texts.push(format!("{head}{body}"));
        // The event's creature is "it" after "attacks"; the sentence names it "that creature".
        let spoken = body.replace(" it.", " that creature.").replace(" it ", " that creature ");
        if let Some(joined) = and_its_controller(&spoken).filter(|_| hits_both(named)) {
            texts.push(format!("{head}{joined}"));
        }
        texts.push(format!("{head}{spoken}"));
    }
    texts
}

/// Whether the effects are the same blow to the event's creature and to its controller.
fn hits_both(ability: &Ability) -> bool {
    let Ok(effects) = serde_json::to_value(ability.effects()) else {
        return false;
    };
    let Some([first, second]) = effects
        .pointer("/0/Together/effects")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return false;
    };
    let amount = first.pointer("/DealDamage/amount");
    effects.as_array().map(Vec::len) == Some(1)
        && first.pointer("/DealDamage/to") == Some(&json!("Triggering"))
        && second.pointer("/DealDamage/to") == Some(&json!({"ControllerOf": "Triggering"}))
        && amount.is_some()
        && second.pointer("/DealDamage/amount") == amount
        && first.as_object().and_then(|m| m.get("DealDamage")).and_then(Value::as_object).map(serde_json::Map::len) == Some(2)
        && second.as_object().and_then(|m| m.get("DealDamage")).and_then(Value::as_object).map(serde_json::Map::len) == Some(2)
}

/// "~ deals 3 damage to that creature and 3 damage to that creature's controller." for the two blows the renderers
/// write as two sentences.
fn and_its_controller(body: &str) -> Option<String> {
    let (first, second) = body.split_once(". Then ")?;
    let amount = first.strip_prefix("~ deals ")?.strip_suffix(" damage to that creature")?;
    let wanted = format!("~ deals {amount} damage to that player.");
    (second == wanted).then(|| {
        format!("~ deals {amount} damage to that creature and {amount} damage to that creature's controller.")
    })
}

/// Whether the targets and effects of `dealt` are those of `named` with the event's creature named as the dealer.
fn same_but_dealer(named: &Ability, dealt: &Ability) -> bool {
    let body = |ability: &Ability| json!([ability.targets(), ability.effects()]);
    let mut renamed = body(named);
    rename(&mut renamed);
    let theirs = body(dealt);
    renamed == theirs && body(named) != theirs
}

/// Names the dealer of the event wherever the value names its subject.
fn rename(value: &mut Value) {
    match value {
        Value::String(word) if word == "Triggering" => *word = "Dealer".to_owned(),
        Value::Array(items) => items.iter_mut().for_each(rename),
        Value::Object(map) => map.values_mut().for_each(rename),
        _ => {}
    }
}

/// "Prevent all [combat] damage that would be dealt to and dealt by enchanted creature.": a shield on damage to the
/// creature attached to and one on damage that creature deals.
fn shield_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (Ok(first), Ok(second)) = (serde_json::to_value(first), serde_json::to_value(second))
    else {
        return Vec::new();
    };
    let to = first.pointer("/Replacement/when/Damage");
    let by = second.pointer("/Replacement/when/Damage");
    let (Some(Value::Object(to)), Some(Value::Object(by))) = (to, by) else {
        return Vec::new();
    };
    // A shield of nothing but its event and the prevention.
    let prevents = |value: &Value| {
        value.pointer("/Replacement/by") == Some(&json!("Prevent"))
            && value
                .pointer("/Replacement")
                .and_then(Value::as_object)
                .map(serde_json::Map::len)
                == Some(2)
    };
    let combat = to.get("combat") == Some(&json!(true));
    let only = |map: &serde_json::Map<String, Value>, key: &str| {
        map.keys().all(|name| name == key || name == "combat")
            && map.get("combat") == to.get("combat")
    };
    let attached_creature = json!([{"Type": "Creature"}, {"Is": "Attached"}]);
    if !prevents(&first)
        || !prevents(&second)
        || to.get("to") != Some(&json!("Attached"))
        || by.get("source") != Some(&attached_creature)
        || !only(to, "to")
        || !only(by, "source")
    {
        return Vec::new();
    }
    let kind = if combat { "combat damage" } else { "damage" };
    ["enchanted", "equipped"]
        .iter()
        .map(|holder| {
            format!("Prevent all {kind} that would be dealt to and dealt by {holder} creature.")
        })
        .collect()
}
