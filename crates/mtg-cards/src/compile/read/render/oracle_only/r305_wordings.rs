//! Wordings of pack r30-5: a creature held back after it attacked during your last turn ("This creature can't attack
//! if it attacked during your last turn."), power and toughness counted from counters or creatures on the battlefield,
//! a cast prohibition during combat, spells cast from other zones that cost more or less, cards that can't enter the
//! battlefield from graveyards and libraries, and a few abilities of one exact shape each.

use serde_json::{Value, json};

use super::super::super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::refs::Selector;

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let found = attacked_last_turn(&value)
        .into_iter()
        .chain(counters_on_group(&value))
        .chain(counted_on_battlefield(&value))
        .chain(cant_cast_in_combat(&value))
        .chain(cast_from_elsewhere(&value))
        .chain(per_lost_player(&value))
        .chain(no_legend_rule(&value))
        .chain(fixed(&value).map(str::to_owned))
        .chain(all_colors(&value));
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "This creature can't attack if it attacked during your last turn." and "This creature doesn't untap during your
/// untap step if it attacked during your last turn.": a static of the source alone, while it attacked last turn.
fn attacked_last_turn(value: &Value) -> Option<String> {
    let condition = json!({ "Matches": { "subject": "This", "selector": ["AttackedLastTurn"] } });
    let verb = ["CantAttack", "DoesntUntap"].into_iter().find(|m| {
        value == &json!({ "Static": { "affected": "This", "mods": [m], "condition": condition } })
    })?;
    let verb = if verb == "CantAttack" {
        "can't attack"
    } else {
        "doesn't untap during your untap step"
    };
    Some(format!("~ {verb} if it attacked during your last turn."))
}

/// "~'s power and toughness are each equal to the number of +1/+1 counters on creatures you control.": a defining
/// power and toughness, both the counters of one kind on a group of permanents.
fn counters_on_group(value: &Value) -> Option<String> {
    let set = value.pointer("/Static/mods/0/SetPT")?;
    let shaped = value.pointer("/Static/affected") == Some(&json!("This"))
        && value.pointer("/Static/defining") == Some(&json!(true))
        && value
            .pointer("/Static/mods")
            .and_then(Value::as_array)?
            .len()
            == 1
        && value.get("Static")?.as_object()?.len() == 3
        && set.get("power") == set.get("toughness");
    let counters = set.pointer("/power/Counters").filter(|_| shaped)?;
    let kind = counters.get("counter")?.as_str()?;
    let group: Selector = serde_json::from_value(counters.pointer("/on/Each")?.clone()).ok()?;
    let group = phrase_text(&group, Form::All)?;
    Some(format!(
        "~'s power and toughness are each equal to the number of {kind} counters on {group}."
    ))
}

/// "~'s power is equal to the number of red creatures on the battlefield.": a defining power or toughness, the count
/// of a group on the battlefield.
fn counted_on_battlefield(value: &Value) -> Option<String> {
    let statik = value.get("Static")?.as_object()?;
    if statik.get("affected") != Some(&json!("This"))
        || statik.get("defining") != Some(&json!(true))
        || statik.len() != 3
    {
        return None;
    }
    let mods = statik
        .get("mods")?
        .as_array()
        .filter(|mods| mods.len() == 1)?;
    let (stat, amount) = mods
        .first()?
        .get("SetPower")
        .map(|amount| ("power", amount))
        .or_else(|| {
            mods.first()?
                .get("SetToughness")
                .map(|amount| ("toughness", amount))
        })?;
    let group: Selector = serde_json::from_value(amount.get("Count")?.clone()).ok()?;
    let group = phrase_text(&group, Form::All)?;
    Some(format!(
        "~'s {stat} is equal to the number of {group} on the battlefield."
    ))
}

/// "Your opponents can't cast spells during combat.": a cast prohibition that is open outside combat.
fn cant_cast_in_combat(value: &Value) -> Option<String> {
    let cant = value.pointer("/Rule/rule/Cant")?;
    let shaped = value.get("Rule")?.as_object()?.len() == 1
        && cant.get("action") == Some(&json!({ "Cast": { "unless": { "Not": "InCombat" } } }));
    let who = match cant.get("who")?.as_str().filter(|_| shaped)? {
        "Opponents" => "Your opponents",
        "EachPlayer" => "Players",
        "You" => "You",
        _ => return None,
    };
    Some(format!("{who} can't cast spells during combat."))
}

/// "Spells your opponents cast from graveyards or from exile cost {2} more to cast.", "Spells you cast from your
/// graveyard or from exile cost {2} less to cast.": a cost change for spells cast from a graveyard or from exile.
fn cast_from_elsewhere(value: &Value) -> Option<String> {
    let modifier = value.get("CostModifier")?.as_object()?;
    if modifier.len() != 3 {
        return None;
    }
    let zones = json!({ "AnyOf": [{ "CastFrom": "Graveyard" }, { "CastFrom": "Exile" }] });
    let (whose, graveyard) = ["You", "Opponents"].into_iter().find_map(|who| {
        (modifier.get("applies_to")? == &json!(["Spell", { "ControlledBy": who }, zones]))
            .then_some(who)
            .map(|who| {
                if who == "You" {
                    ("you", "your graveyard")
                } else {
                    ("your opponents", "graveyards")
                }
            })
    })?;
    let change = match modifier.get("change")?.as_str()? {
        "Reduce" => "less",
        "Increase" => "more",
        _ => return None,
    };
    let mana = modifier.get("mana")?.as_str()?;
    Some(format!(
        "Spells {whose} cast from {graveyard} or from exile cost {mana} {change} to cast."
    ))
}

/// "This creature gets +10/+10 for each player who has lost the game.": a bonus of the source counted from the players
/// out of the game.
fn per_lost_player(value: &Value) -> Option<String> {
    let statik = value.get("Static")?.as_object()?;
    let pump = statik.get("mods")?.pointer("/0/ModifyPT")?;
    let times = |key: &str| {
        let times = pump.get(key)?.get("Times")?;
        (times.get("of")? == "LostPlayers").then(|| times.get("by")?.as_i64())?
    };
    let (power, toughness) = (times("power")?, times("toughness")?);
    let shaped = statik.len() == 2
        && statik.get("affected")? == "This"
        && statik.get("mods")?.as_array()?.len() == 1
        && power > 0
        && toughness > 0;
    shaped.then(|| format!("~ gets +{power}/+{toughness} for each player who has lost the game."))
}

/// "~ is all colors.": the defining colors of a face that names no color identity.
fn all_colors(value: &Value) -> Option<String> {
    let shape = json!({ "Static": {
        "affected": "This",
        "mods": [{ "SetColors": ["White", "Blue", "Black", "Red", "Green"] }],
        "defining": true
    } });
    (value == &shape).then(|| "~ is all colors.".to_owned())
}

/// "The \"legend rule\" doesn't apply to creature tokens you control.": the legend rule lifted for a group.
fn no_legend_rule(value: &Value) -> Option<String> {
    let rule = value.get("Rule")?.as_object()?;
    let group: Selector =
        serde_json::from_value(rule.get("rule")?.get("NoLegendRuleFor")?.clone()).ok()?;
    let group = phrase_text(&group, Form::All).filter(|_| rule.len() == 1)?;
    Some(format!("The \"legend rule\" doesn't apply to {group}."))
}

/// The wording of an ability of one exact shape.
fn fixed(value: &Value) -> Option<&'static str> {
    let permanent = json!({ "AnyOf": [
        { "Type": "Artifact" }, { "Type": "Battle" }, { "Type": "Creature" },
        { "Type": "Enchantment" }, { "Type": "Land" }, { "Type": "Planeswalker" }
    ] });
    let enter = |who: Value| json!({ "Rule": { "rule": { "Cant": { "who": who, "action": "EnterBattlefield" } } } });
    let nonland =
        |zone: &str| json!({ "Each": [{ "Not": { "Type": "Land" } }, permanent, { "In": zone }] });
    let table = [
        (
            json!({ "Rule": { "rule": { "Cant": {
                "who": "EachPlayer",
                "action": { "Cast": { "filter": [{ "SharesName": { "Each": [{ "Not": "Token" }] } }] } }
            } } } }),
            "Players can't cast spells with the same name as a nontoken permanent.",
        ),
        (
            json!({ "CostModifier": {
                "applies_to": ["This"],
                "change": "Reduce",
                "mana": "{1}",
                "per": { "Sum": [{ "AnyCounters": "EachPlayer" }, { "AnyCounters": { "Each": [] } }] }
            } }),
            "This spell costs {1} less to cast for each counter among players and permanents.",
        ),
        (
            enter(json!({ "Each": [permanent, { "In": "Graveyard" }] })),
            "Permanent cards in graveyards can't enter the battlefield.",
        ),
        (
            enter(json!({ "Union": [nonland("Graveyard"), nonland("Library")] })),
            "Nonland permanent cards in graveyards and libraries can't enter the battlefield.",
        ),
    ];
    table
        .into_iter()
        .chain(plain_shapes())
        .find(|(shape, _)| shape == value)
        .map(|(_, text)| text)
}

/// The shapes of `fixed` that need no shared part.
fn plain_shapes() -> [(Value, &'static str); 8] {
    [
        (
            json!({ "Replacement": { "when": { "Damage": { "source": ["This"] } }, "by": { "Multiply": 2 } } }),
            "Double all damage ~ would deal.",
        ),
        (
            json!({ "Rule": {
            "rule": { "Combat": { "who": "This", "quirk": "AttackAsThoughHaste" } },
            "condition": { "Not": { "Matches": { "subject": "This", "selector": ["EnteredThisTurn"] } } }
        } }),
            "~ can attack as though it had haste unless it entered this turn.",
        ),
        (
            json!({ "Static": {
            "affected": "This",
            "mods": [{ "SetColors": ["White", "Blue", "Black", "Red", "Green"] }],
            "defining": true
        } }),
            "~ is all colors. This ability doesn't affect its color identity.",
        ),
        (
            json!({ "Rule": { "rule": { "Cant": { "who": "EachPlayer", "action": { "Cast": { "filter": [
            { "Not": { "Type": "Creature" } },
            { "AnyOf": [{ "In": "Graveyard" }, { "In": "Exile" }] }
        ] } } } } } }),
            "Players can't cast noncreature spells from graveyards or exile.",
        ),
        (
            json!({ "Static": {
            "affected": { "Each": [{ "Type": "Creature" }, { "AnyOf": [
                { "Subtype": "Saproling" },
                { "Not": { "AnyOf": [{ "Not": { "Subtype": "Treefolk" } }, "This"] } }
            ] }] },
            "mods": [{ "ModifyPT": { "power": 1, "toughness": 1 } }]
        } }),
            "Saproling creatures and other Treefolk creatures get +1/+1.",
        ),
        (
            json!({ "Triggered": {
            "when": { "DealsDamage": {
                "source": [{ "Type": "Creature" }, { "ControlledBy": "You" }],
                "combat": true
            } },
            "if": { "AtTrigger": "YourTurn" },
            "effects": [{ "PutCounters": { "on": "Dealer", "counter": "+1/+1", "count": "Triggering" } }]
        } }),
            "Whenever a creature you control deals combat damage during your turn, put that many +1/+1 counters on it.",
        ),
        (
            json!({ "Rule": {
            "rule": { "MayCast": {
                "what": ["This"],
                "from": "Graveyard",
                "additional": [{ "RemoveCountersAmong": { "count": 6, "from": [{ "Type": "Creature" }] } }]
            } },
            "from": "Graveyard"
        } }),
            "You may cast this card from your graveyard by removing six counters from among creatures you control in \
         addition to paying its other costs.",
        ),
        (
            json!({ "Triggered": {
            "when": { "ZoneChange": { "what": ["This"], "to": "Battlefield" } },
            "if": { "CastWith": "Escape" },
            "effects": [{ "Sacrifice": {
                "who": "Opponents",
                "filter": [{ "Type": "Creature" }, { "Not": { "Subtype": "Gorgon" } }]
            } }]
        } }),
            "When it enters this way, each opponent sacrifices a non-Gorgon creature of their choice.",
        ),
    ]
}
