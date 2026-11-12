//! Wordings of pack r30-2c: whole spells no other writer words, each from the shape its effects take: an exchange of
//! two targets that share a card type, a life auction for control of a target, one exile per player that its
//! controller pays for with a cloak, a look at the top cards that plots one and keeps the rest, and a shield that
//! doubles the damage a target permanent deals.

use serde_json::{Value, json};

use super::amounts::count_word;
use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition};

/// Adds the wordings of this module for the ability to `texts`, each once.
pub(super) fn r302c_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(value) = serde_json::to_value(ability) else {
        return;
    };
    let found = [
        exchange_sharing(&value),
        auction(&value),
        exile_each_cloak(&value),
        look_plot(&value),
        double_from_target(&value),
        uncounterable_if(ability),
        kicked_split_second(&value),
    ];
    for text in found.into_iter().flatten() {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// A selector of the data as the typed value.
fn selector(value: &Value) -> Option<Selector> {
    serde_json::from_value(value.clone()).ok()
}

/// "Exchange control of two target nonland permanents that share a card type.": a first target of `base` and another of
/// `base` that shares a card type with it.
fn exchange_sharing(value: &Value) -> Option<String> {
    let base = value.pointer("/Spell/targets/0/what/Object")?;
    let mut second = base.as_array()?.clone();
    second.push(json!({ "SharesType": { "Target": 0 } }));
    let shape = json!({ "Spell": {
        "targets": [
            { "what": { "Object": base } },
            { "what": { "Another": { "what": { "Object": second }, "earlier": [0] } } }
        ],
        "effects": [{ "ExchangeControl": { "what": [{ "Target": 0 }, { "Target": 1 }], "duration": "Forever" } }]
    } });
    (value == &shape).then_some(())?;
    let what = match base.as_array()?.as_slice() {
        [] => "permanents".to_owned(),
        [single] if single == &json!({ "Not": { "Type": "Land" } }) => {
            "nonland permanents".to_owned()
        }
        _ => phrase_text(&selector(base)?, Form::Targets)?,
    };
    Some(format!(
        "Exchange control of two target {what} that share a card type."
    ))
}

/// "Each player may bid life for control of target creature. You start the bidding with a bid of 0. ... The high bidder
/// loses life equal to the high bid and gains control of the creature."
fn auction(value: &Value) -> Option<String> {
    let what = value.pointer("/Spell/targets/0/what/Object")?;
    let start = value.pointer("/Spell/effects/0/BidLife/start")?.as_u64()?;
    let shape = json!({ "Spell": {
        "targets": [{ "what": { "Object": what } }],
        "effects": [
            { "BidLife": { "start": start, "as": "bid", "winner": "bidder" } },
            { "LoseLife": { "who": { "Bound": "bidder" }, "amount": { "ChosenAs": "bid" } } },
            { "Continuous": {
                "affected": { "Target": 0 },
                "mods": [{ "SetController": { "Bound": "bidder" } }],
                "duration": "Forever"
            } }
        ]
    } });
    (value == &shape).then_some(())?;
    let noun = phrase_text(&selector(what)?, Form::Target)?;
    let object = match what.as_array()?.as_slice() {
        [single] if single == &json!({ "Type": "Creature" }) => "creature",
        _ => "permanent",
    };
    Some(format!(
        "Each player may bid life for control of target {noun}. You start the bidding with a bid of {start}. In turn \
         order, each player may top the high bid. The bidding ends if the high bid stands. The high bidder loses life \
         equal to the high bid and gains control of the {object}."
    ))
}

/// "For each player, exile up to one target nonland permanent that player controls. For each permanent exiled this way,
/// its controller cloaks the top card of their library."
fn exile_each_cloak(value: &Value) -> Option<String> {
    let what = value.pointer("/Spell/targets/0/what/PerPlayer/what")?;
    let shape = json!({ "Spell": {
        "targets": [{
            "what": { "PerPlayer": { "players": "EachPlayer", "what": what } },
            "bounds": { "min": 0, "max": 1 }
        }],
        "effects": [
            { "Exile": { "what": { "Target": 0 }, "as": "exiled" } },
            { "RepeatEach": {
                "over": { "Bound": "exiled" },
                "as": "card",
                "effects": [{ "Cloak": { "who": { "ControllerOf": { "Departed": { "Bound": "card" } } } } }]
            } }
        ]
    } });
    (value == &shape).then_some(())?;
    let noun = match what.as_array()?.as_slice() {
        [] => "permanent".to_owned(),
        [single] if single == &json!({ "Not": { "Type": "Land" } }) => {
            "nonland permanent".to_owned()
        }
        _ => phrase_text(&selector(what)?, Form::Target)?,
    };
    Some(format!(
        "For each player, exile up to one target {noun} that player controls. For each permanent exiled this way, its \
         controller cloaks the top card of their library."
    ))
}

/// "Look at the top three cards of your library. You may exile a nonland card from among them. If you do, it becomes
/// plotted. Put the rest into your hand."
fn look_plot(value: &Value) -> Option<String> {
    let count = value
        .pointer("/Spell/effects/0/LookAtTop/count")?
        .as_u64()
        .filter(|count| *count > 1)?;
    let shape = json!({ "Spell": { "effects": [
        { "LookAtTop": { "count": count, "as": "top" } },
        { "Choose": {
            "from": { "Each": [{ "In": "Library" }, { "Is": { "Bound": "top" } }, { "Not": { "Type": "Land" } }] },
            "count": { "min": 0, "max": 1 },
            "as": "kept"
        } },
        { "Plot": { "what": { "Bound": "kept" } } },
        { "Move": { "what": { "Bound": "top" }, "to": "Hand" } }
    ] } });
    (value == &shape).then_some(())?;
    let count = count_word(u32::try_from(count).ok()?)?;
    Some(format!(
        "Look at the top {count} cards of your library. You may exile a nonland card from among them. If you do, it \
         becomes plotted. Put the rest into your hand."
    ))
}

/// "Each time target permanent would deal damage to a permanent or player this turn, it deals double that damage to
/// that permanent or player instead."
fn double_from_target(value: &Value) -> Option<String> {
    let shape = json!({ "Spell": {
        "targets": [{ "what": { "Object": [] } }],
        "effects": [{ "Shield": {
            "when": { "Damage": { "source": [{ "Is": { "Target": 0 } }] } },
            "by": { "Multiply": 2 },
            "duration": "UntilEndOfTurn"
        } }]
    } });
    (value == &shape).then_some(())?;
    Some(
        "Each time target permanent would deal damage to a permanent or player this turn, it deals double that damage \
         to that permanent or player instead."
            .to_owned(),
    )
}

/// "If you revealed a Dragon card or controlled a Dragon as you cast this spell, this spell can't be countered.": the
/// rule that keeps the spell on the stack under the condition of an optional reveal.
fn uncounterable_if(ability: &Ability) -> Option<String> {
    let Ability::Rule {
        rule: GameRule::CantBeCountered(Ref::This),
        condition: Some(condition),
        from: ZoneKind::Stack,
    } = ability
    else {
        return None;
    };
    let condition = super::e16b_presence::text(condition)?;
    Some(format!("If {condition}, this spell can't be countered."))
}

/// "If X is 5 or more, this spell can't be countered and the damage can't be prevented.": two rules of the spell on the
/// stack under one condition on X.
pub(super) fn r302c_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Rule {
            rule: GameRule::CantBeCountered(Ref::This),
            condition: Some(condition),
            from: ZoneKind::Stack,
        },
        Ability::Rule {
            rule:
                GameRule::DamageCantBePrevented {
                    source,
                    combat: None,
                },
            condition: Some(again),
            from: ZoneKind::Stack,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    if condition != again || source.0 != [Pred::This] {
        return Vec::new();
    }
    let Condition::Compare {
        left: Amount::X,
        op: Cmp::Ge,
        right: Amount::Fixed(least),
    } = condition
    else {
        return Vec::new();
    };
    vec![format!(
        "If X is {least} or more, this spell can't be countered and the damage can't be prevented."
    )]
}

/// "If this spell was kicked, it has split second.": the spell gives itself the keyword on the stack.
fn kicked_split_second(value: &Value) -> Option<String> {
    let shape = json!({ "Static": {
        "affected": { "Each": ["This", { "In": "Stack" }] },
        "mods": [{ "AddKeyword": "SplitSecond" }],
        "condition": "Kicked",
        "from": "Stack"
    } });
    (value == &shape).then(|| "If this spell was kicked, it has split second.".to_owned())
}
