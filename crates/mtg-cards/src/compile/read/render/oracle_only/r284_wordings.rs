//! Wordings of r28-4's shapes: a target spell or permanent given colors for good or the color of your choice until end
//! of turn ("Target spell or permanent becomes black.", "Target spell or permanent becomes colorless.", "Target spell or
//! permanent becomes the color of your choice until end of turn."), a target creature made to attack or block this turn
//! ("Target creature attacks this turn if able."), a target that dealt damage this turn and a target changed and then
//! dealt damage ("... ~ deals damage to that creature equal to ...").

use serde_json::{Value, json};

use super::super::written_variants;
use crate::ability::Ability;
use crate::compile::read::conditional::condition_texts;
use crate::value::Condition;

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    if let Some(text) = spell_or_permanent_becomes(&value) {
        texts.push(text);
    }
    if let Some(text) = must_this_turn(&value) {
        texts.push(text);
    }
    let more = dealt_damage_target(&value)
        .into_iter()
        .chain(two_steps_on_target(&value))
        .chain(flash_if(&value))
        .chain(sacrifice_one_that_dealt_damage(&value))
        .chain(exiled_card_to_graveyard(&value));
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
        .map(|text| {
            let text = text.strip_prefix("if ").unwrap_or(&text).to_owned();
            format!("If {text}, you may cast this spell as though it had flash.")
        })
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
