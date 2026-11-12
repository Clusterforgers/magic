//! Wordings no other writer gives: a creature held back by its attack during its controller's last turn, power and
//! toughness counted from the counters on a group, a cost counted from every counter in the game, the legend rule
//! lifted for a group, an attack as though with haste unless the creature entered this turn, an anthem over one kind
//! and other creatures of a second kind, a power or toughness defined alone, and "must be blocked by exactly one
//! creature if able" as a requirement beside a limit, counters an escaped creature enters with beside what it does as it
//! enters this way, a counter moved onto "that creature", a cost changed for each target of the spell, a bonus for each
//! player who has lost the game, lands put onto the battlefield from among the cards a trigger milled, and a spell that
//! costs less if a player attacked you during their last turn, spells that target the enchanted player costing less,
//! a reduction for each creature type that can't reduce the cost by more than {5}, and a Case solved with seven lands
//! that lets its controller look at and play from the top of their library.

use serde_json::{Value, json};

use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::refs::Selector;

/// Adds the wordings of this module for the ability to `texts`, each once.
pub(super) fn r305b_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let found = [fixed(&value), counters_define(&value), legend_rule(&value)];
    for text in found.into_iter().flatten() {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The two abilities of one sentence: "~ must be blocked by exactly one creature if able."
pub(super) fn r305b_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (Ok(first), Ok(second)) = (serde_json::to_value(first), serde_json::to_value(second))
    else {
        return Vec::new();
    };
    let must = json!({ "Rule": { "rule": { "Combat": { "who": "This", "quirk": { "MustBeBlocked": "AtLeastOne" } } } } });
    let limit = json!({ "Static": { "affected": "This", "mods": [{ "MaxBlockers": 1 }] } });
    if first == must && second == limit {
        return vec!["~ must be blocked by exactly one creature if able".to_owned()];
    }
    let escapes = json!({ "Replacement": {
        "when": { "Enters": { "what": ["This"] } },
        "by": { "WithCounters": { "counter": "+1/+1", "count": 2 } },
        "condition": { "CastWith": "Escape" }
    } });
    let sacrifice = json!({ "Triggered": {
        "when": { "ZoneChange": { "what": ["This"], "to": "Battlefield" } },
        "if": { "CastWith": "Escape" },
        "effects": [{ "Sacrifice": { "who": "Opponents", "filter": [{ "Type": "Creature" }, { "Not": { "Subtype": "Gorgon" } }] } }]
    } });
    let solved = json!({ "Matches": { "subject": "This", "selector": ["Solved"] } });
    let look = json!({ "Rule": { "rule": { "LookAtTopAnyTime": "You" }, "condition": solved } });
    let play = json!({ "Rule": {
        "rule": { "MayCast": {
            "what": [{ "AnyOf": [{ "Type": "Land" }, { "Type": "Creature" }, { "Type": "Enchantment" }] }],
            "from": "Library",
            "play": true
        } },
        "condition": solved
    } });
    if first == look && second == play {
        return vec![
            "Solved \u{2014} You may look at the top card of your library any time, and you may play lands and cast creature and enchantment spells from the top of your library"
                .to_owned(),
        ];
    }
    if first == escapes && second == sacrifice {
        return vec![
            "~ escapes with two +1/+1 counters on it. When it enters this way, each opponent sacrifices a non-Gorgon creature of their choice"
                .to_owned(),
        ];
    }
    Vec::new()
}

/// A static whose condition is that the card matches one atom.
fn held_if(mods: &Value, atom: &str) -> Value {
    json!({ "Static": {
        "affected": "This",
        "mods": mods,
        "condition": { "Matches": { "subject": "This", "selector": [atom] } }
    } })
}

/// The wordings of whole abilities this module knows by shape.
#[allow(clippy::too_many_lines)]
fn fixed(value: &Value) -> Vec<String> {
    let counted = |color: &str| json!({ "Count": [{ "Color": color }, { "Type": "Creature" }] });
    let defined = |stat: &str, color: &str| json!({ "Static": { "affected": "This", "mods": [{ stat: counted(color) }], "defining": true } });
    let treefolk = json!({ "Each": [
        { "Type": "Creature" },
        { "AnyOf": [{ "Subtype": "Saproling" }, { "Not": { "AnyOf": [{ "Not": { "Subtype": "Treefolk" } }, "This"] } }] }
    ] });
    let haste = json!({ "Rule": {
        "rule": { "Combat": { "who": "This", "quirk": "AttackAsThoughHaste" } },
        "condition": { "Not": { "Matches": { "subject": "This", "selector": ["EnteredThisTurn"] } } }
    } });
    let every_counter = json!({ "CostModifier": {
        "applies_to": ["This"], "change": "Reduce", "mana": "{1}",
        "per": { "AnyCounters": { "Union": ["EachPlayer", { "Each": [] }] } }
    } });
    let from_elsewhere = json!({ "CostModifier": {
        "applies_to": ["Spell", { "ControlledBy": "Opponents" }, { "AnyOf": [{ "CastFrom": "Graveyard" }, { "CastFrom": "Exile" }] }],
        "change": "Increase", "mana": "{2}"
    } });
    let move_one = json!({ "Triggered": {
        "when": { "ZoneChange": { "what": [{ "Type": "Creature" }, { "ControlledBy": "You" }], "to": "Battlefield" } },
        "effects": [{ "May": { "effects": [{ "MoveCounters": { "from": "This", "to": "Triggering" } }] } }]
    } });
    let per_target = |who: &str, change: &str| {
        json!({ "CostModifier": {
            "applies_to": ["Spell", { "ControlledBy": who }], "change": change, "mana": "{1}", "per": "CastTargets"
        } })
    };
    let lost = json!({ "Times": { "by": 10, "of": "PlayersLost" } });
    let frogantua = json!({ "Static": { "affected": "This", "mods": [{ "ModifyPT": { "power": lost, "toughness": lost } }] } });
    let milled_lands = json!({ "Triggered": {
        "when": { "DealsDamage": { "source": ["This"], "to": "EachPlayer", "combat": true } },
        "effects": [{ "May": { "effects": [
            { "Mill": { "count": "Triggering", "as": "milled" } },
            { "Choose": {
                "from": { "Each": [{ "In": "Graveyard" }, { "Is": { "Bound": "milled" } }, { "Type": "Land" }] },
                "count": { "min": 0, "max": null },
                "as": "lands"
            } },
            { "Move": { "what": { "Bound": "lands" }, "to": { "Battlefield": { "tapped": true } } } }
        ] } }]
    } });
    let avenge = json!({ "CostModifier": {
        "applies_to": ["This"], "change": "Reduce", "mana": "{2}",
        "condition": { "AttackedYouLastTurn": "EachPlayer" }
    } });
    let at_enchanted = json!({ "CostModifier": {
        "applies_to": ["Spell", { "ControlledBy": "You" }, { "TargetsPlayer": "AttachedPlayer" }],
        "change": "Reduce", "mana": "{1}"
    } });
    let changeling = json!({ "CostModifier": {
        "applies_to": ["This"], "change": "Reduce", "mana": "{1}",
        "per": { "AtMost": {
            "of": { "Distinct": { "of": [{ "Type": "Creature" }, { "ControlledBy": "You" }], "stat": "CreatureType" } },
            "most": 5
        } }
    } });
    let to_solve = json!({ "Triggered": {
        "when": { "StepBegins": { "step": "End", "whose": "You" } },
        "if": { "All": [
            { "Not": { "Matches": { "subject": "This", "selector": ["Solved"] } } },
            { "Compare": { "left": { "Count": [{ "Type": "Land" }, { "ControlledBy": "You" }] }, "op": "Ge", "right": 7 } }
        ] },
        "effects": [{ "Solve": { "what": "This" } }]
    } });
    let text = if value == &to_solve {
        "To solve \u{2014} You control seven or more lands"
    } else if value == &changeling {
        "This spell costs {1} less to cast for each creature type among creatures you control. This effect can't reduce the amount of mana this spell costs by more than {5}"
    } else if value == &at_enchanted {
        "Spells you cast that target enchanted player cost {1} less to cast"
    } else if value == &avenge {
        "This spell costs {2} less to cast if a player attacked you during their last turn"
    } else if value == &per_target("You", "Reduce") {
        "Spells you cast cost {1} less to cast for each target"
    } else if value == &per_target("Opponents", "Increase") {
        "Spells your opponents cast cost {1} more to cast for each target"
    } else if value == &frogantua {
        "~ gets +10/+10 for each player who has lost the game"
    } else if value == &milled_lands {
        "Whenever ~ deals combat damage to a player, you may mill that many cards. Put any number of land cards from among them onto the battlefield tapped"
    } else if value == &move_one {
        "Whenever a creature you control enters, you may move a counter from ~ onto that creature"
    } else if value == &from_elsewhere {
        "Spells your opponents cast from graveyards or from exile cost {2} more to cast"
    } else if value == &held_if(&json!(["CantAttack"]), "AttackedLastTurn") {
        "~ can't attack if it attacked during your last turn"
    } else if value == &held_if(&json!(["DoesntUntap"]), "AttackedLastTurn") {
        "~ doesn't untap during your untap step if it attacked during your last turn"
    } else if value == &defined("SetPower", "Red") {
        "~'s power is equal to the number of red creatures on the battlefield"
    } else if value == &defined("SetToughness", "Green") {
        "~'s toughness is equal to the number of green creatures on the battlefield"
    } else if value
        == &json!({ "Static": { "affected": treefolk, "mods": [{ "ModifyPT": { "power": 1, "toughness": 1 } }] } })
    {
        "Saproling creatures and other Treefolk creatures get +1/+1"
    } else if value == &haste {
        "~ can attack as though it had haste unless it entered this turn"
    } else if value == &every_counter {
        "This spell costs {1} less to cast for each counter among players and permanents"
    } else {
        return Vec::new();
    };
    vec![text.to_owned()]
}

/// "~'s power and toughness are each equal to the number of +1/+1 counters on creatures you control."
fn counters_define(value: &Value) -> Vec<String> {
    let Some(body) = value.get("Static") else {
        return Vec::new();
    };
    if body.get("affected") != Some(&json!("This"))
        || body.get("defining") != Some(&json!(true))
        || body.get("condition").is_some()
    {
        return Vec::new();
    }
    let Some(set) = body
        .get("mods")
        .and_then(Value::as_array)
        .filter(|mods| mods.len() == 1)
        .and_then(|mods| mods[0].get("SetPT"))
    else {
        return Vec::new();
    };
    let (Some(power), Some(toughness)) = (set.get("power"), set.get("toughness")) else {
        return Vec::new();
    };
    if power != toughness {
        return Vec::new();
    }
    let Some(counters) = power.get("Counters") else {
        return Vec::new();
    };
    if counters.get("counter") != Some(&json!("+1/+1")) {
        return Vec::new();
    }
    let Some(group) = counters
        .get("on")
        .and_then(|on| on.get("Each"))
        .and_then(|each| serde_json::from_value::<Selector>(each.clone()).ok())
        .filter(|group| !group.0.is_empty())
        .and_then(|group| phrase_text(&group, Form::All))
    else {
        return Vec::new();
    };
    vec![format!(
        "~'s power and toughness are each equal to the number of +1/+1 counters on {group}"
    )]
}

/// "The \"legend rule\" doesn't apply to creature tokens you control."
fn legend_rule(value: &Value) -> Vec<String> {
    let Some(group) = value
        .get("Rule")
        .filter(|rule| rule.get("condition").is_none() && rule.get("from").is_none())
        .and_then(|rule| rule.get("rule"))
        .and_then(|rule| rule.get("NoLegendRuleFor"))
        .and_then(|group| serde_json::from_value::<Selector>(group.clone()).ok())
    else {
        return Vec::new();
    };
    phrase_text(&group, Form::All)
        .map(|group| format!("The \"legend rule\" doesn't apply to {group}"))
        .into_iter()
        .collect()
}
