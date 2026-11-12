//! Wordings of pack l29-0: the activation of an exhaust or a boast ability, a payment of mana and a discard, and whole
//! triggered abilities whose head or steps no other writer words, each written from a twin the other writers word.

use serde_json::{Value, json};

use super::costs;
use super::triggers::player_verb;
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::did::Action;
use crate::keyword::Keyword;
use crate::refs::{Pred, Ref};
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// "Whenever you activate an exhaust ability", "Whenever you activate a boast ability".
pub(super) fn l290_did_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::Did { action, who, what } = pattern else {
        return None;
    };
    let kind = match action {
        Action::ActivatedExhaust => "an exhaust ability",
        Action::ActivatedBoast => "a boast ability",
        _ => return None,
    };
    what.is_empty().then_some(())?;
    Some(format!("Whenever {} {kind}", player_verb(who, "activate")?))
}

/// "Whenever you become the target of a spell or ability an opponent controls", "Whenever you become the target of a
/// spell".
pub(super) fn l290_player_targeted_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::PlayerBecomesTarget { who, by } = pattern else {
        return None;
    };
    let what = match by.0.as_slice() {
        [] => "a spell or ability",
        [Pred::ControlledBy(Ref::Opponents)] => "a spell or ability an opponent controls",
        [Pred::ControlledBy(Ref::You)] => "a spell or ability you control",
        [Pred::Spell] => "a spell",
        [Pred::Spell, Pred::ControlledBy(Ref::Opponents)] => "a spell an opponent controls",
        _ => return None,
    };
    Some(format!(
        "Whenever {} the target of {what}",
        player_verb(who, "become")?
    ))
}

/// "Whenever you attack the player who has the initiative": the attacked player tested for the initiative.
pub(super) fn l290_attacks_initiative_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::PlayerAttacks { by, at } = pattern else {
        return None;
    };
    let initiative = json!({"PlayersWhere": {"who": "EachPlayer", "test": {"Initiative": "You"}}});
    (serde_json::to_value(at).ok()? == initiative).then_some(())?;
    Some(format!(
        "Whenever {} the player who has the initiative",
        player_verb(by, "attack")?
    ))
}

/// "pay {1} and discard a card": mana and a discard of any cards, paid together.
pub(super) fn l290_pay_phrase(cost: &Cost) -> Option<String> {
    let [
        CostItem::Mana(mana),
        discard @ CostItem::Discard {
            count: Amount::Fixed(1..),
            filter: None,
        },
    ] = cost.0.as_slice()
    else {
        return None;
    };
    (!mana.has_x()).then_some(())?;
    let discarded = costs::text(&Cost(vec![discard.clone()]))?;
    let mut chars = discarded.chars();
    let first = chars.next()?;
    Some(format!(
        "pay {mana} and {}{}",
        first.to_lowercase(),
        chars.as_str()
    ))
}

/// Adds the wordings of whole triggered abilities no other writer words: a keyword gained when the power is high
/// enough, an attack while you have the most life, the land token that is every basic land type, a land that becomes a Plains for as long as the source stays, and an attack with the
/// card and/or your commander.
pub(super) fn l290_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(body) = value.get("Triggered") else {
        return;
    };
    let mut found = Vec::new();
    let untargeted = body.get("targets").is_none();
    if untargeted
        && let Some(tail) = gains_if_power(&body["effects"])
        && let Some(head) = head(body, written)
    {
        found.push(format!("{head} {tail}."));
    }
    if untargeted
        && body["effects"]
            == json!([{"CreateToken": {"token": {"Custom": everywhere()}, "tapped": true}}])
        && let Some(head) = head(body, written)
    {
        found.push(format!(
            "{head} create a tapped colorless land token named Everywhere that is every basic land type."
        ));
    }
    if let Some(tail) = becomes_land_type(body)
        && let Some(head) = head(body, written)
    {
        found.push(format!("{head} {tail}."));
    }
    // "while you have the most life or are tied for most life": checked as it triggers.
    if untargeted
        && body["if"] == json!({"AtTrigger": {"Among": "MostLife"}})
        && body["when"] == json!({"Attacks": {"what": ["This"]}})
        && let Some(steps) = you_steps(&body["effects"])
    {
        found.push(format!(
            "Whenever ~ attacks while you have the most life or are tied for most life, {steps}."
        ));
    }
    if body["when"] == your_attack_with_commander()
        && let Some(rest) = commander_attack_rest(body, written)
    {
        found.push(format!(
            "Whenever you attack with ~ and/or your commander, {rest}"
        ));
    }
    if untargeted
        && let Some(tail) = attacking_copies(&body["effects"])
        && let Some(head) = head(body, written)
    {
        found.push(format!("{head} {tail}."));
    }
    // An activation that targets a creature or player, copied (Ertha Jo).
    let targets_creature_or_player = json!({"AtTrigger": {"Matches": {"subject": "Dealer", "selector": [
        {"AnyOf": [{"Targeting": [{"Type": "Creature"}]}, {"TargetsPlayer": "EachPlayer"}]}
    ]}}});
    if body["when"] == json!({"Did": {"action": "Activated", "who": "You"}})
        && body["if"] == targets_creature_or_player
        && body["effects"] == json!([{"CopyAbility": {"what": "Dealer", "new_targets": true}}])
        && untargeted
    {
        found.push(
            "Whenever you activate an ability that targets a creature or player, copy that ability. You may \
             choose new targets for the copy."
                .to_owned(),
        );
    }
    // "Whenever this enchantment or a legendary creature you control enters": the card's own entering and another kind's.
    if let Some([own, _]) = body["when"]["AnyOf"].as_array().map(Vec::as_slice)
        && *own == json!({"ZoneChange": {"what": ["This"], "to": "Battlefield"}})
    {
        found.extend(texts.iter().filter_map(|text| {
            let rest = text.strip_prefix("When ~ enters and whenever ")?;
            Some(format!(
                "Whenever ~ or {}",
                rest.replacen(", you create ", ", create ", 1)
            ))
        }));
    }
    // "if you don't control a creature named Keimi": an intervening "if" on a count reads as the step it guards.
    if texts.is_empty()
        && found.is_empty()
        && body["if"]["Compare"]["left"].get("Count").is_some()
        && let Some(text) = intervening_count(body, written)
    {
        found.push(text);
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// Tapped and attacking token copies that leave at end of combat: of the card the source exiled, an Illusion as well
/// (Phantom Steed), or of each attacking modified creature you control (Mirror-Style Master).
fn attacking_copies(effects: &Value) -> Option<String> {
    let end_of_combat = |verb: &str| {
        json!({"DelayedTrigger": {
            "when": {"StepBegins": {"step": "EndCombat"}},
            "effects": [match verb {
                "Sacrifice" => json!({"Sacrifice": {"filter": [{"Is": {"Bound": "copy"}}]}}),
                _ => json!({"Exile": {"what": {"Bound": "copy"}}}),
            }]
        }})
    };
    let exiled = json!([
        {"CreateToken": {
            "token": {"CopyOf": {
                "of": {"Each": [{"ExiledWith": "This"}, {"In": "Exile"}]},
                "except": [{"AddSubtype": "Illusion"}]
            }},
            "tapped": true, "attacking": true, "as": "copy"
        }},
        end_of_combat("Sacrifice")
    ]);
    if *effects == exiled {
        return Some(
            "create a tapped and attacking token that's a copy of the exiled card, except it's an Illusion in \
             addition to its other types. Sacrifice that token at end of combat"
                .to_owned(),
        );
    }
    let each_modified = json!([{"RepeatEach": {
        "over": {"Each": [{"Type": "Creature"}, "Attacking", "Modified", {"ControlledBy": "You"}]},
        "as": "c",
        "effects": [
            {"CreateToken": {
                "token": {"CopyOf": {"of": {"Bound": "c"}}},
                "tapped": true, "attacking": true, "as": "copy"
            }},
            end_of_combat("Exile")
        ]
    }}]);
    (*effects == each_modified).then(|| {
        "for each attacking modified creature you control, create a tapped and attacking token that's a copy of \
         that creature. Exile those tokens at end of combat"
            .to_owned()
    })
}

/// The wording of the twin whose condition guards its steps as one `If`: the same words as the intervening "if".
fn intervening_count(body: &Value, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let mut twin = body.clone();
    let map = twin.as_object_mut()?;
    let held = map.remove("if")?;
    let effects = map.remove("effects")?;
    map.insert(
        "effects".to_owned(),
        json!([{"If": {"condition": held, "then": effects}}]),
    );
    let twin: Ability = serde_json::from_value(json!({ "Triggered": twin })).ok()?;
    written(&twin).into_iter().next()
}

/// The head of the trigger as its twin with one plain step words it: "Whenever ~ attacks,".
fn head(body: &Value, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let mut twin = body.clone();
    let map = twin.as_object_mut()?;
    map.remove("targets");
    map.insert("effects".to_owned(), json!([{"Draw": {}}]));
    let twin: Ability = serde_json::from_value(json!({ "Triggered": twin })).ok()?;
    written(&twin)
        .into_iter()
        .find_map(|text| text.strip_suffix(" draw a card.").map(str::to_owned))
}

/// "it gains trample until end of turn if its power is 10 or greater".
fn gains_if_power(effects: &Value) -> Option<String> {
    let [step] = effects.as_array()?.as_slice() else {
        return None;
    };
    let test = &step["If"];
    let n = test["condition"]["Matches"]["selector"][0]["Compare"]["to"].as_u64()?;
    let power = json!({"Matches": {"subject": "This", "selector": [
        {"Compare": {"stat": "Power", "op": "Ge", "to": n}}
    ]}});
    (test["condition"] == power && test.get("otherwise").is_none()).then_some(())?;
    let [gain] = test["then"].as_array()?.as_slice() else {
        return None;
    };
    let granted = gain["Continuous"]["mods"][0]["AddKeyword"].clone();
    let keyword: Keyword = serde_json::from_value(granted.clone()).ok()?;
    let expected = json!({"Continuous": {
        "affected": "This",
        "mods": [{"AddKeyword": granted}],
        "duration": "UntilEndOfTurn"
    }});
    (*gain == expected).then_some(())?;
    Some(format!(
        "it gains {} until end of turn if its power is {n} or greater",
        super::selectors::keyword_text(&keyword)?.to_lowercase()
    ))
}

/// "you draw a card and you lose 1 life": a draw and a loss of life, each said with its subject.
fn you_steps(effects: &Value) -> Option<String> {
    let [draw, lose] = effects.as_array()?.as_slice() else {
        return None;
    };
    let n = lose["LoseLife"]["amount"].as_u64()?;
    (*draw == json!({"Draw": {}}) && *lose == json!({"LoseLife": {"amount": n}}))
        .then(|| format!("you draw a card and you lose {n} life"))
}

/// The land token named Everywhere: colorless and every basic land type.
fn everywhere() -> Value {
    json!({
        "name": "Everywhere",
        "types": ["Land"],
        "subtypes": ["Plains", "Island", "Swamp", "Mountain", "Forest"]
    })
}

/// "you may have target land become a Plains until ~ leaves the battlefield".
fn becomes_land_type(body: &Value) -> Option<String> {
    (body["targets"] == json!([{"what": {"Object": [{"Type": "Land"}]}}])).then_some(())?;
    let effects = &body["effects"];
    let land =
        effects[0]["May"]["effects"][0]["Continuous"]["mods"][0]["SetLandTypes"][0].as_str()?;
    let expected = json!([{"May": {"effects": [{"Continuous": {
        "affected": {"Target": 0},
        "mods": [{"SetLandTypes": [land]}],
        "duration": "WhileSourceOnBattlefield"
    }}]}}]);
    (*effects == expected).then_some(())?;
    let article = if land.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    Some(format!(
        "you may have target land become {article} {land} until ~ leaves the battlefield"
    ))
}

/// "Whenever you attack with ~ and/or your commander": creatures you control among the card and the commanders you own.
fn your_attack_with_commander() -> Value {
    json!({"GroupAttacks": {"what": [
        {"ControlledBy": "You"},
        {"AnyOf": ["This", {"Is": {"Each": ["Commander", {"OwnedBy": "You"}]}}]}
    ]}})
}

/// What follows "Whenever you attack," in the twin that triggers on any attack of yours.
fn commander_attack_rest(body: &Value, written: fn(&Ability) -> Vec<String>) -> Option<String> {
    let mut twin = body.clone();
    twin.as_object_mut()?.insert(
        "when".to_owned(),
        json!({"GroupAttacks": {"what": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}),
    );
    let twin: Ability = serde_json::from_value(json!({ "Triggered": twin })).ok()?;
    written(&twin).into_iter().find_map(|text| {
        text.strip_prefix("Whenever you attack, ")
            .map(str::to_owned)
    })
}
