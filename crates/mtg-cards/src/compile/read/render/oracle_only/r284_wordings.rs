//! Wordings of r28-4's shapes: a target spell or permanent given colors for good or the color of your choice until end
//! of turn ("Target spell or permanent becomes black.", "Target spell or permanent becomes colorless.", "Target spell or
//! permanent becomes the color of your choice until end of turn."), a target creature made to attack or block this turn
//! ("Target creature attacks this turn if able."), a target that dealt damage this turn and a target changed and then
//! dealt damage ("... ~ deals damage to that creature equal to ..."), and a few abilities of one exact shape each.

use serde_json::{Value, json};

use super::super::written_variants;
use crate::ability::Ability;
use crate::compile::read::conditional::condition_texts;
use crate::compile::read::selectors::{Form, phrase_text};
use crate::modify::{GameRule, SpendAs};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Condition;

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    if let Some(text) = spell_or_permanent_becomes(&value) {
        texts.push(text);
    }
    texts.extend(cast_from_graveyard(ability));
    texts.extend(while_attacked_with(&value));
    texts.extend(fixed_wordings(&value));
    if let Some(text) = must_this_turn(&value) {
        texts.push(text);
    }
    let more = dealt_damage_target(&value)
        .into_iter()
        .chain(two_steps_on_target(&value))
        .chain(flash_if(&value))
        .chain(sacrifice_one_that_dealt_damage(&value))
        .chain(exiled_card_to_graveyard(&value))
        .chain(found_gains_indestructible(&value))
        .chain(search_library_and_graveyard(&value));
    for text in more {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "Spell mastery — If there are two or more instant and/or sorcery cards in your graveyard, you may cast this spell as
/// though it had flash.": the condition of a rule that lets the card be cast from the hand as though it had flash, put
/// first.
fn flash_if(value: &Value) -> Vec<String> {
    let Some(rule) = value.get("Rule").and_then(Value::as_object) else {
        return Vec::new();
    };
    let flash = json!({ "MayCast": { "what": ["This"], "flash": true } });
    let shaped = rule.len() == 3
        && rule.get("rule") == Some(&flash)
        && rule.get("from") == Some(&json!("Hand"));
    let Some(condition) = rule
        .get("condition")
        .filter(|_| shaped)
        .and_then(|condition| serde_json::from_value::<Condition>(condition.clone()).ok())
    else {
        return Vec::new();
    };
    condition_texts(&condition)
        .into_iter()
        .flat_map(|text| {
            let text = text.strip_prefix("if ").unwrap_or(&text).to_owned();
            // A card that is an instant or a sorcery is "instant and/or sorcery" in the Oracle text.
            let and_or = text.replace("instant or sorcery cards", "instant and/or sorcery cards");
            [text, and_or]
        })
        .map(|text| format!("If {text}, you may cast this spell as though it had flash."))
        .collect()
}

/// "As an additional cost to cast this spell, sacrifice a creature that dealt damage this turn.": the wordings of the
/// cost with the creature picked without the damage it dealt, "that dealt damage this turn" put after its noun.
fn sacrifice_one_that_dealt_damage(value: &Value) -> Vec<String> {
    let Some(extra) = value.get("AdditionalCost") else {
        return Vec::new();
    };
    let dealt = json!({ "DidThisTurn": { "event": "Damaged" } });
    let creature = json!({ "Type": "Creature" });
    let Some([part]) = extra
        .get("cost")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Vec::new();
    };
    let Some(sacrifice) = part.get("Sacrifice").and_then(Value::as_object) else {
        return Vec::new();
    };
    if sacrifice.len() != 1 || sacrifice.get("filter") != Some(&json!([creature, dealt])) {
        return Vec::new();
    }
    let mut twin = extra.clone();
    twin["cost"] = json!([{ "Sacrifice": { "filter": [creature] } }]);
    let Ok(twin) = serde_json::from_value::<Ability>(json!({ "AdditionalCost": twin })) else {
        return Vec::new();
    };
    written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches("sacrifice a creature").count() == 1)
        .map(|text| {
            text.replacen(
                "sacrifice a creature",
                "sacrifice a creature that dealt damage this turn",
                1,
            )
        })
        .collect()
}

/// "As an additional cost to cast this spell, put a card an opponent owns from exile into that player's graveyard.": a
/// required cost of one card an opponent owns put from exile into its owner's graveyard.
fn exiled_card_to_graveyard(value: &Value) -> Option<String> {
    let shape = json!({
        "AdditionalCost": {
            "cost": [{ "ExiledToGraveyard": { "filter": [{ "OwnedBy": "Opponents" }] } }],
            "required": true
        }
    });
    (value == &shape).then(|| {
        "As an additional cost to cast this spell, put a card an opponent owns from exile into that player's graveyard."
            .to_owned()
    })
}

/// "Search your library for a Dinosaur creature card, put it onto the battlefield, then shuffle. It gains indestructible
/// until your next turn.": a spell whose last step gives the card an earlier step found indestructible until your next
/// turn, worded as the steps before it and then that sentence.
fn found_gains_indestructible(value: &Value) -> Vec<String> {
    let Some(spell) = value.get("Spell") else {
        return Vec::new();
    };
    let Some((last, before)) = spell
        .get("effects")
        .and_then(Value::as_array)
        .and_then(|steps| steps.split_last())
    else {
        return Vec::new();
    };
    let Some(found) = before
        .first()
        .and_then(|first| first.get("Search"))
        .and_then(|search| search.get("as"))
    else {
        return Vec::new();
    };
    let gives = json!({ "Continuous": {
        "affected": { "Bound": found },
        "mods": [{ "AddKeyword": "Indestructible" }],
        "duration": "UntilYourNextTurn"
    } });
    if last != &gives {
        return Vec::new();
    }
    let targets = spell.get("targets").cloned().unwrap_or_else(|| json!([]));
    let Some(head) = spell_with(&targets, before) else {
        return Vec::new();
    };
    written_variants(&head)
        .into_iter()
        .map(|text| format!("{text} It gains indestructible until your next turn."))
        .collect()
}

/// "Search your library and/or graveyard for an Ally creature card, reveal it, and put it into your hand. If you search
/// your library this way, shuffle.": a spell whose steps search the library and the graveyard, worded as the same steps
/// of an enters trigger without "When ~ enters, ".
fn search_library_and_graveyard(value: &Value) -> Vec<String> {
    let Some(spell) = value.get("Spell").and_then(Value::as_object) else {
        return Vec::new();
    };
    let Some(effects) = spell.get("effects").and_then(Value::as_array) else {
        return Vec::new();
    };
    let searches_graveyard = effects
        .first()
        .and_then(|first| first.get("Search"))
        .and_then(|search| search.get("also"))
        == Some(&json!(["Graveyard"]));
    if spell.len() != 1 || !searches_graveyard {
        return Vec::new();
    }
    let twin = json!({ "Triggered": {
        "when": { "ZoneChange": { "what": ["This"], "to": "Battlefield" } },
        "effects": effects
    } });
    let Ok(twin) = serde_json::from_value::<Ability>(twin) else {
        return Vec::new();
    };
    written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let rest = text.strip_prefix("When ~ enters, ")?;
            let mut letters = rest.chars();
            let first = letters.next()?.to_uppercase().to_string();
            Some(format!("{first}{}", letters.as_str()))
        })
        .collect()
}

/// The ability `value` with its spell's targets and steps replaced, as an ability.
fn spell_with(targets: &Value, effects: &[Value]) -> Option<Ability> {
    serde_json::from_value(json!({ "Spell": { "targets": targets, "effects": effects } })).ok()
}

/// "Target creature that dealt damage this turn gets -5/-5 until end of turn.": the wordings of the spell with its one
/// target picked without the damage it dealt, "that dealt damage this turn" put after the target's noun.
fn dealt_damage_target(value: &Value) -> Vec<String> {
    let dealt = json!({ "DidThisTurn": { "event": "Damaged" } });
    let creature = json!({ "Type": "Creature" });
    let Some(spell) = value.get("Spell") else {
        return Vec::new();
    };
    let Some(effects) = spell.get("effects").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some([target]) = spell
        .get("targets")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Vec::new();
    };
    let Some(preds) = target
        .get("what")
        .and_then(|what| what.get("Object"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    if preds.as_slice() != [creature.clone(), dealt]
        || target.as_object().map(serde_json::Map::len) != Some(1)
    {
        return Vec::new();
    }
    let plain = json!([{ "what": { "Object": [creature] } }]);
    let Some(twin) = spell_with(&plain, effects) else {
        return Vec::new();
    };
    written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches("arget creature").count() == 1)
        .map(|text| {
            text.replacen(
                "Target creature",
                "Target creature that dealt damage this turn",
                1,
            )
            .replacen(
                "target creature ",
                "target creature that dealt damage this turn ",
                1,
            )
        })
        .filter(|text| text.contains("that dealt damage this turn"))
        .collect()
}

/// "Target creature with flying loses flying until end of turn. ~ deals damage to that creature equal to the number of
/// Forests you control.": a change and then damage to one target creature, each worded alone and the second naming it
/// "that creature".
fn two_steps_on_target(value: &Value) -> Vec<String> {
    let Some(spell) = value.get("Spell") else {
        return Vec::new();
    };
    let Some([target]) = spell
        .get("targets")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Vec::new();
    };
    let Some([first, second]) = spell
        .get("effects")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Vec::new();
    };
    let creature = json!({ "Type": "Creature" });
    let is_creature = target
        .get("what")
        .and_then(|what| what.get("Object"))
        .and_then(Value::as_array)
        .is_some_and(|preds| preds.first() == Some(&creature));
    let first_target = json!({ "Target": 0 });
    let names_target = |step: &Value| step.to_string().contains(&first_target.to_string());
    let shaped = first.get("Continuous").is_some() && second.get("DealDamage").is_some();
    if !shaped || !is_creature || !names_target(first) || !names_target(second) {
        return Vec::new();
    }
    let targets = json!([target]);
    let plain = json!([{ "what": { "Object": [creature] } }]);
    let (Some(head), Some(tail)) = (
        spell_with(&targets, std::slice::from_ref(first)),
        spell_with(&plain, std::slice::from_ref(second)),
    ) else {
        return Vec::new();
    };
    let heads: Vec<String> = written_variants(&head)
        .into_iter()
        .filter(|text| text.starts_with("Target "))
        .take(2)
        .collect();
    let tails: Vec<String> = written_variants(&tail)
        .into_iter()
        .filter(|text| text.matches("target creature").count() == 1 && !text.contains("Target"))
        .map(|text| text.replacen("target creature", "that creature", 1))
        .take(4)
        .collect();
    heads
        .iter()
        .flat_map(|head| tails.iter().map(move |tail| format!("{head} {tail}")))
        .collect()
}

/// "As long as ~ is on the battlefield, you may cast Knight spells from your graveyard.": a permanent's plain permission
/// to cast spells of a kind from its controller's graveyard, which works while it is on the battlefield.
fn cast_from_graveyard(ability: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let plain = permission.who == Ref::You
        && permission.from == Some(ZoneKind::Graveyard)
        && !permission.free
        && !permission.flash
        && !permission.play
        && !permission.once
        && permission.only.is_none()
        && permission.additional.is_none()
        && permission.rather.is_none()
        && permission.when.is_none()
        && permission.spend_as == SpendAs::Normal
        && !permission.what.0.is_empty();
    if !plain {
        return Vec::new();
    }
    let mut preds = vec![Pred::Spell];
    preds.extend(permission.what.0.iter().cloned());
    let Some(spells) = phrase_text(&Selector(preds), Form::All) else {
        return Vec::new();
    };
    vec![
        format!("As long as ~ is on the battlefield, you may cast {spells} from your graveyard."),
        format!("You may cast {spells} from your graveyard."),
    ]
}

/// "As long as you attacked with three or more Merfolk this turn, Merfolk you control get +1/+0.": a static ability under
/// the condition that its controller attacked with at least N creatures of a kind this turn, worded as the ability
/// without it after that condition.
fn while_attacked_with(value: &Value) -> Vec<String> {
    const NUMBERS: [&str; 9] = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    let Some(body) = value.get("Static").and_then(Value::as_object) else {
        return Vec::new();
    };
    let Some(compare) = body
        .get("condition")
        .and_then(|condition| condition.get("Compare"))
    else {
        return Vec::new();
    };
    let Some(tally) = compare.get("left").and_then(|left| left.get("Tally")) else {
        return Vec::new();
    };
    let counted = tally.get("event") == Some(&json!("Attacked"))
        && tally.get("who") == Some(&json!("You"))
        && compare.get("op") == Some(&json!("Ge"));
    let number = compare
        .get("right")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .and_then(|n| n.checked_sub(1))
        .and_then(|at| NUMBERS.get(at));
    let kind = tally
        .get("filter")
        .and_then(|filter| serde_json::from_value::<Selector>(filter.clone()).ok())
        .filter(|filter| matches!(filter.0.as_slice(), [Pred::Subtype(_)]))
        .and_then(|filter| phrase_text(&filter, Form::All));
    let (Some(number), Some(kind)) = (number.filter(|_| counted), kind) else {
        return Vec::new();
    };
    let mut twin = body.clone();
    twin.remove("condition");
    let Ok(twin) = serde_json::from_value::<Ability>(json!({ "Static": twin })) else {
        return Vec::new();
    };
    written_variants(&twin)
        .into_iter()
        .filter(|text| !text.starts_with('~'))
        .map(|text| {
            // A common word that opens the sentence is lowercase after the condition; a type's name keeps its capital.
            let first = text.split(' ').next().unwrap_or_default();
            let common = ["Each", "Other", "All", "Creatures", "Target", "You"].contains(&first);
            let text = if common {
                let mut letters = text.chars();
                let head = letters
                    .next()
                    .map(|c| c.to_lowercase().to_string())
                    .unwrap_or_default();
                format!("{head}{}", letters.as_str())
            } else {
                text
            };
            format!("As long as you attacked with {number} or more {kind} this turn, {text}")
        })
        .collect()
}

/// Abilities of one exact shape and their one wording: the shapes below say all they do in words of their own.
fn fixed_wordings(value: &Value) -> Vec<String> {
    let melting = json!({ "Static": { "affected": { "Each": [{ "Type": "Land" }] }, "mods": [{ "RemoveSupertype": "Snow" }] } });
    let sideswipe = json!({ "Spell": {
        "targets": [{ "what": { "Object": ["Spell", { "Subtype": "Arcane" }] } }],
        "effects": [{ "ChangeTargets": { "what": { "Target": 0 }, "to": "Choose" } }]
    } });
    let plagiarize = json!({ "Spell": {
        "targets": [{ "what": "Player" }],
        "effects": [{ "Shield": {
            "when": { "Draw": { "who": { "Target": 0 } } },
            "by": { "Instead": [{ "Draw": {} }] },
            "duration": "UntilEndOfTurn"
        } }]
    } });
    let revealed = json!({ "Bound": "revealed" });
    let ignite = json!({ "Spell": {
        "targets": [{ "what": "Player" }],
        "effects": [
            { "ChooseAtRandom": { "from": { "Each": [{ "OwnedBy": { "Target": 0 } }, { "In": "Hand" }] }, "as": "revealed" } },
            { "Reveal": { "what": revealed } },
            { "DealDamage": { "to": { "Target": 0 }, "amount": { "Stat": { "of": revealed, "stat": "ManaValue" } } } }
        ]
    } });
    let tapped_disguise = json!({ "Static": {
        "affected": "This",
        "mods": [
            { "SetCreatureTypes": ["Human", "Citizen"] },
            { "SetPT": { "power": 1, "toughness": 1 } },
            "CantBeBlocked"
        ],
        "condition": { "Matches": { "subject": "This", "selector": ["Tapped"] } }
    } });
    let creature = json!({ "Bound": "creature" });
    let breach = json!({ "Spell": { "effects": [{ "May": { "effects": [
        { "Choose": { "from": { "Each": [{ "In": "Hand" }, { "OwnedBy": "You" }, { "Type": "Creature" }] }, "as": "creature" } },
        { "Move": { "what": creature, "to": { "Battlefield": {} } } },
        { "Continuous": { "affected": creature, "mods": [{ "AddKeyword": "Haste" }], "duration": "Forever" } },
        { "DelayedTrigger": {
            "when": { "StepBegins": { "step": "End" } },
            "effects": [{ "Sacrifice": { "filter": [{ "Is": creature }] } }]
        } }
    ] } }] } });
    let text = if value == &breach {
        "You may put a creature card from your hand onto the battlefield. That creature gains haste. Sacrifice that \
         creature at the beginning of the next end step."
    } else if value == &melting {
        "All lands are no longer snow."
    } else if value == &sideswipe {
        "You may change any targets of target Arcane spell."
    } else if value == &plagiarize {
        "Until end of turn, if target player would draw a card, instead that player skips that draw and you draw a card."
    } else if value == &ignite {
        "Target player reveals a card at random from their hand. ~ deals damage to that player equal to that card's mana value."
    } else if value == &tapped_disguise {
        "As long as ~ is tapped, it's a Human Citizen with base power and toughness 1/1 and can't be blocked."
    } else {
        return Vec::new();
    };
    // The card's own name is "this creature" in newer Oracle text.
    let named = text.replacen("~ is tapped", "this creature is tapped", 1);
    if named == text {
        vec![text.to_owned()]
    } else {
        vec![text.to_owned(), named]
    }
}

/// "Target creature attacks this turn if able." and "Target creature blocks this turn if able." for a spell whose one
/// step makes its one target, a creature, attack or block until end of turn.
fn must_this_turn(value: &Value) -> Option<String> {
    let spell = value.get("Spell")?.as_object()?;
    let target = json!([{ "what": { "Object": [{ "Type": "Creature" }] } }]);
    if spell.len() != 2 || spell.get("targets")? != &target {
        return None;
    }
    let [step] = spell.get("effects")?.as_array()?.as_slice() else {
        return None;
    };
    let rule = step.get("Rule")?.as_object()?;
    if rule.len() != 2 || rule.get("duration")? != "UntilEndOfTurn" {
        return None;
    }
    let first = json!({ "Target": 0 });
    let verb = if rule.get("rule")?.get("MustAttack") == Some(&first) {
        "attacks"
    } else if rule.get("rule")?.get("MustBlock") == Some(&first) {
        "blocks"
    } else {
        return None;
    };
    Some(format!("Target creature {verb} this turn if able."))
}

/// "Target spell or permanent becomes <colors>." for a spell whose one target is a spell or a permanent and whose one
/// step sets its colors for good, or "... becomes the color of your choice until end of turn." for a color chosen and
/// then given until end of turn.
fn spell_or_permanent_becomes(value: &Value) -> Option<String> {
    let spell = value.get("Spell")?.as_object()?;
    if spell.len() != 2 {
        return None;
    }
    let target =
        json!([{ "what": { "Object": [{ "AnyOf": ["Spell", { "In": "Battlefield" }] }] } }]);
    if spell.get("targets")? != &target {
        return None;
    }
    let words = match spell.get("effects")?.as_array()?.as_slice() {
        [given] => {
            let colors = given_mods(given, "Forever")?.get("SetColors")?.as_array()?;
            colors_words(colors)?
        }
        [choose, given] => {
            let chosen = choose.get("ChooseValue")?.as_object()?;
            let name = chosen.get("as")?;
            let only_choice = chosen.len() == 2 && chosen.get("kind")? == "Color";
            let mods = given_mods(given, "UntilEndOfTurn")?;
            (only_choice && mods.get("SetColorsChosen")? == name)
                .then(|| "the color of your choice until end of turn".to_owned())?
        }
        _ => return None,
    };
    Some(format!("Target spell or permanent becomes {words}."))
}

/// The one modification a `Continuous` step gives the first target for `duration`.
fn given_mods<'a>(step: &'a Value, duration: &str) -> Option<&'a Value> {
    let given = step.get("Continuous")?.as_object()?;
    let shaped = given.len() == 3
        && given.get("affected")? == &json!({ "Target": 0 })
        && given.get("duration")? == duration;
    match given.get("mods")?.as_array()?.as_slice() {
        [one] if shaped => Some(one),
        _ => None,
    }
}

/// "black", "white and blue", "colorless" for the colors a step sets.
fn colors_words(colors: &[Value]) -> Option<String> {
    let words: Vec<String> = colors
        .iter()
        .map(|color| color.as_str().map(str::to_lowercase))
        .collect::<Option<_>>()?;
    match words.as_slice() {
        [] => Some("colorless".to_owned()),
        [one] => Some(one.clone()),
        _ => None,
    }
}
