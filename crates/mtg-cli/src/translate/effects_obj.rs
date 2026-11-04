//! Effects on objects: destroy, counter, pump, counters, tap, sacrifice, fight and gain control.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, only, unmapped};
use super::keyword::{keyword_list, keyword_value};
use super::remember;
use super::valid::{defined, objects, selector_any, subject};
use crate::forge::script::Params;

/// The effects of a verb on objects.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let one = |value: Tr<Value>| value.map(|value| vec![value]);
    match api {
        "Destroy" => one(destroy(ctx, p)),
        "DestroyAll" => one(destroy_all(ctx, p)),
        "DamageAll" => damage_all(ctx, p),
        "PutCounterAll" => one(put_counter_all(ctx, p)),
        "Counter" => one(counter(ctx, p)),
        "PreventDamage" => one(prevent_damage(ctx, p)),
        "Pump" => pump(ctx, p),
        "PumpAll" => pump_all(ctx, p),
        "Animate" | "AnimateAll" => animate(ctx, p),
        "PutCounter" => one(put_counter(ctx, p)),
        "RemoveCounter" => one(remove_counter(ctx, p)),
        "Tap" => tap(ctx, p),
        "Untap" => one(simple(ctx, p, "Untap", "Untap")),
        "Regenerate" => one(simple(ctx, p, "Regenerate", "Regenerate")),
        "TapAll" => one(all_of(ctx, p, "Tap")),
        "UntapAll" => one(all_of(ctx, p, "Untap")),
        "Sacrifice" => one(sacrifice(ctx, p)),
        "Fight" => one(fight(ctx, p)),
        "GainControl" => gain_control(ctx, p),
        other => unmapped(format!("effect:{other}")),
    }
}

/// How long an effect lasts: its `Duration$` word, until end of turn when it has none.
pub fn duration(word: Option<&str>) -> Tr<&'static str> {
    match word {
        None | Some("EOT") => Ok("UntilEndOfTurn"),
        Some("Permanent") => Ok("Forever"),
        Some("UntilEndOfCombat") => Ok("UntilEndOfCombat"),
        Some("UntilYourNextTurn") => Ok("UntilYourNextTurn"),
        Some("UntilTheEndOfYourNextTurn") => Ok("UntilEndOfYourNextTurn"),
        Some("UntilHostLeavesPlay") => Ok("WhileSourceOnBattlefield"),
        Some(other) => unmapped(format!("duration:{other}")),
    }
}

fn simple(ctx: &mut Ctx, p: &Params, api: &str, verb: &str) -> Tr<Value> {
    only(p, api, &["ValidTgts", "Defined", "TargetMin", "TargetMax"])?;
    let what = subject(ctx, p, "Self", None)?;
    Ok(json!({ verb: {"what": what} }))
}

/// `Tap`: the target or `Defined$` object, or "tap any number of" the objects `CardChoices$` allows, which the controller
/// picks on resolution. `RememberTapped$` binds the permanents that became tapped. A cap other than "all that match"
/// stays a gap.
fn tap(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let remember = remember::wanted(p, &["RememberTapped"]);
    let Some(choices) = p.get("CardChoices") else {
        let mut effect = simple(ctx, &p.without(&["RememberTapped"]), "Tap", "Tap")?;
        if remember {
            remember::bind_moved(ctx, &mut effect, "Tap", "tapped");
        }
        return Ok(vec![effect]);
    };
    only(
        p,
        "Tap",
        &[
            "CardChoices",
            "AnyNumber",
            "ChoiceAmount",
            "ChoicePrompt",
            "RememberTapped",
        ],
    )?;
    let all = format!("Count$Valid {choices}");
    if p.get("AnyNumber") != Some("True") || p.get("ChoiceAmount") != Some(all.as_str()) {
        return unmapped("key:Tap.CardChoices-amount");
    }
    let filter = selector_any(ctx, choices)?;
    let max = json!({"Count": filter});
    let from = json!({"Each": filter});
    let bind = ctx.fresh("tap");
    let mut effect = json!({"Tap": {"what": {"Bound": bind}}});
    if remember {
        remember::bind_moved(ctx, &mut effect, "Tap", "tapped");
    }
    let pick = json!({"Choose": {"from": from, "count": {"min": 0, "max": max}, "as": bind}});
    Ok(vec![pick, effect])
}

/// `TapAll`, `UntapAll`: the verb on every object the valid-string names.
fn all_of(ctx: &mut Ctx, p: &Params, verb: &str) -> Tr<Value> {
    only(p, verb, &["ValidCards"])?;
    let valid = p.get("ValidCards").unwrap_or_default();
    Ok(json!({ verb: {"what": {"Each": selector_any(ctx, valid)?}} }))
}

fn destroy(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Destroy",
        &[
            "ValidTgts",
            "Defined",
            "NoRegen",
            "TargetMin",
            "TargetMax",
            "RememberDestroyed",
            "RememberTargets",
        ],
    )?;
    let what = subject(ctx, p, "", None)?;
    let regenerate = p.get("NoRegen") != Some("True");
    let mut effect = json!({"Destroy": {"what": what, "regenerate": regenerate}});
    if remember::wanted(p, &["RememberDestroyed", "RememberTargets"]) {
        remember::bind_moved(ctx, &mut effect, "Destroy", "destroyed");
    }
    Ok(effect)
}

fn destroy_all(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "DestroyAll",
        &["ValidCards", "NoRegen", "RememberDestroyed"],
    )?;
    let valid = p.get("ValidCards").unwrap_or_default();
    let regenerate = p.get("NoRegen") != Some("True");
    let what = json!({"Each": selector_any(ctx, valid)?});
    let mut effect = json!({"Destroy": {"what": what, "regenerate": regenerate}});
    if remember::wanted(p, &["RememberDestroyed"]) {
        remember::bind_moved(ctx, &mut effect, "Destroy", "destroyed");
    }
    Ok(effect)
}

fn damage_all(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "DamageAll", &["NumDmg", "ValidCards", "ValidPlayers"])?;
    let valid = p.get("ValidCards").unwrap_or_default();
    let amount = amount(ctx, p.get("NumDmg").unwrap_or("1"))?;
    let mut effects =
        vec![json!({"DealDamage": {"to": {"Each": selector_any(ctx, valid)?}, "amount": amount}})];
    match p.get("ValidPlayers") {
        None => {}
        Some("Player") => {
            effects.push(json!({"DealDamage": {"to": "EachPlayer", "amount": amount}}));
        }
        Some("Opponent" | "Player.Opponent") => {
            effects.push(json!({"DealDamage": {"to": "Opponents", "amount": amount}}));
        }
        Some(other) => return unmapped(format!("key:DamageAll.ValidPlayers={other}")),
    }
    Ok(effects)
}

/// `PutCounterAll`: counters on every object that matches.
fn put_counter_all(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "PutCounterAll",
        &["ValidCards", "CounterType", "CounterNum"],
    )?;
    let Some(kind) = p.get("CounterType").and_then(kind_word) else {
        return unmapped("key:PutCounterAll.CounterType");
    };
    let valid = p.get("ValidCards").unwrap_or_default();
    let count = amount(ctx, p.get("CounterNum").unwrap_or("1"))?;
    Ok(
        json!({"PutCounters": {"on": {"Each": selector_any(ctx, valid)?}, "counter": kind, "count": count}}),
    )
}

fn counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "Counter", &["TargetType", "ValidTgts"])?;
    if p.get("TargetType") != Some("Spell") {
        return unmapped("key:Counter.TargetType");
    }
    let mut selector = vec![json!("Spell")];
    selector.extend(selector_any(ctx, p.get("ValidTgts").unwrap_or("Card"))?);
    let what = ctx.target(json!({"what": {"Object": selector}}))?;
    Ok(json!({"Counter": {"what": what}}))
}

/// `PreventDamage`: the next damage to the target this turn is prevented, up to an amount or all of it.
fn prevent_damage(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "PreventDamage",
        &["Amount", "ValidTgts", "Defined", "TargetMin", "TargetMax"],
    )?;
    let by = match p.get("Amount") {
        Some("All") => json!("Prevent"),
        Some(text) => match amount(ctx, text)?.as_u64() {
            Some(number) => json!({ "PreventUpTo": number }),
            None => return unmapped("key:PreventDamage.Amount-count"),
        },
        None => return unmapped("key:PreventDamage.no-Amount"),
    };
    let to = subject(ctx, p, "", None)?;
    Ok(json!({"Shield": {"when": {"Damage": {"to": to}}, "by": by, "duration": "UntilEndOfTurn"}}))
}

/// The modifications a hidden keyword (`HIDDEN CARDNAME can't block.`) stands for.
fn hidden(word: &str) -> Option<Vec<Value>> {
    let word = word.trim_end_matches('.');
    Some(match word {
        "HIDDEN CARDNAME can't block" => vec![json!("CantBlock")],
        "HIDDEN CARDNAME can't attack" => vec![json!("CantAttack")],
        "HIDDEN CARDNAME can't attack or block" => vec![json!("CantAttack"), json!("CantBlock")],
        "HIDDEN CARDNAME's power and toughness are switched" => vec![json!("SwitchPT")],
        DOESNT_UNTAP => vec![json!("DoesntUntap")],
        _ => return None,
    })
}

/// The hidden keyword whose effect lasts through the next untap step only.
const DOESNT_UNTAP: &str = "HIDDEN This card doesn't untap during your next untap step";

/// The duration of a pump line. "Doesn't untap during your next untap step" lasts through that step, whatever
/// `Duration$ Permanent` says; it stands alone and with that duration only.
fn pump_duration(p: &Params) -> Tr<&'static str> {
    let Some(keywords) = p.get("KW") else {
        return duration(p.get("Duration"));
    };
    if !keywords.contains("doesn't untap during your next untap step") {
        return duration(p.get("Duration"));
    }
    if keywords.trim_end_matches('.') == DOESNT_UNTAP && p.get("Duration") == Some("Permanent") {
        return Ok("NextUntapStep");
    }
    unmapped("keyword:HIDDEN-doesnt-untap-shape")
}

/// The modifications of a pump line: power and toughness, then keywords.
fn pump_mods(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    if p.has("NumAtt") || p.has("NumDef") {
        let power = amount(ctx, p.get("NumAtt").unwrap_or("0"))?;
        let toughness = amount(ctx, p.get("NumDef").unwrap_or("0"))?;
        mods.push(json!({"ModifyPT": {"power": power, "toughness": toughness}}));
    }
    if let Some(keywords) = p.get("KW") {
        for word in keywords.split('&').map(str::trim) {
            match hidden(word) {
                Some(found) => mods.extend(found),
                None => mods.push(json!({ "AddKeyword": keyword_value(ctx, word)? })),
            }
        }
    }
    if mods.is_empty() {
        return unmapped("key:Pump.nothing");
    }
    Ok(mods)
}

fn pump(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Pump",
        &[
            "NumAtt",
            "NumDef",
            "KW",
            "Defined",
            "ValidTgts",
            "Duration",
            "IsCurse",
            "TargetMin",
            "TargetMax",
        ],
    )?;
    let duration = pump_duration(p)?;
    let affected = objects(subject(ctx, p, "Self", None)?)?;
    // A pump that only chooses a target ("target creature you control deals damage ... to target creature") changes
    // nothing itself; the part after it uses the target.
    if !(p.has("NumAtt") || p.has("NumDef") || p.has("KW"))
        && p.has("ValidTgts")
        && p.has("SubAbility")
    {
        return Ok(Vec::new());
    }
    let mods = pump_mods(ctx, p)?;
    Ok(vec![
        json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}),
    ])
}

fn pump_all(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "PumpAll",
        &[
            "NumAtt",
            "NumDef",
            "KW",
            "ValidCards",
            "Duration",
            "IsCurse",
        ],
    )?;
    let duration = pump_duration(p)?;
    let valid = p.get("ValidCards").unwrap_or_default();
    let affected = json!({"Each": selector_any(ctx, valid)?});
    let mods = pump_mods(ctx, p)?;
    Ok(vec![
        json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}),
    ])
}

/// `Animate`: the object becomes something else for a while ("becomes a 4/4 Elemental creature with flying").
fn animate(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Animate",
        &[
            "Defined",
            "ValidTgts",
            "Types",
            "Power",
            "Toughness",
            "Colors",
            "OverwriteColors",
            "Keywords",
            "RemoveKeywords",
            "RemoveAllAbilities",
            "Triggers",
            "Abilities",
            "RemoveCreatureTypes",
            "RemoveCardTypes",
            "RemoveLandTypes",
            "AddAllCreatureTypes",
            "ValidCards",
            "Duration",
            "TargetMin",
            "TargetMax",
        ],
    )?;
    let duration = duration(p.get("Duration"))?;
    // `AnimateAll` names the objects by a valid-string.
    let affected = match p.get("ValidCards") {
        Some(valid) => json!({"Each": selector_any(ctx, valid)?}),
        None => objects(subject(ctx, p, "", None)?)?,
    };
    let mut mods = Vec::new();
    match (p.get("Power"), p.get("Toughness")) {
        (Some(power), Some(toughness)) => {
            let (power, toughness) = (amount(ctx, power)?, amount(ctx, toughness)?);
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
        (None, None) => {}
        _ => return unmapped("key:Animate.half-stats"),
    }
    if let Some(colors) = p.get("Colors") {
        if p.get("OverwriteColors") != Some("True") {
            return unmapped("key:Animate.Colors-added");
        }
        mods.push(json!({"SetColors": color_names(colors)?}));
    }
    mods.extend(type_mods(ctx, p)?);
    if let Some(keywords) = p.get("Keywords") {
        for keyword in keyword_list(ctx, keywords)? {
            mods.push(json!({ "AddKeyword": keyword }));
        }
    }
    mods.extend(super::effects_animate::abilities(ctx, p)?);
    if mods.is_empty() {
        return unmapped("key:Animate.nothing");
    }
    Ok(vec![
        json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}),
    ])
}

/// Colours as the DSL names them.
pub fn color_names(colors: &str) -> Tr<Vec<&'static str>> {
    colors
        .split(',')
        .map(|word| match word.trim() {
            "White" => Ok("White"),
            "Blue" => Ok("Blue"),
            "Black" => Ok("Black"),
            "Red" => Ok("Red"),
            "Green" => Ok("Green"),
            other => unmapped(format!("key:Animate.Colors={other}")),
        })
        .collect()
}

/// The type changes of an animate line: the creature types it sets or adds, then the card types it adds.
///
/// `RemoveCardTypes$` makes the card types the line names the only ones (it needs `Types$`), `RemoveLandTypes$` the basic
/// land types it names the only land types, and `AddAllCreatureTypes$` adds every creature type.
fn type_mods(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    const BASIC_LAND_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];
    let (remove_cards, remove_lands) = (
        p.get("RemoveCardTypes") == Some("True"),
        p.get("RemoveLandTypes") == Some("True"),
    );
    let mut mods = Vec::new();
    if p.get("AddAllCreatureTypes") == Some("True") {
        mods.push(json!("AddAllCreatureTypes"));
    }
    let Some(types) = p.get("Types") else {
        if remove_cards {
            return unmapped("key:Animate.RemoveCardTypes");
        }
        if remove_lands {
            return unmapped("key:Animate.RemoveLandTypes");
        }
        return Ok(mods);
    };
    let (mut card_types, mut subtypes, mut lands, mut supertypes) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for word in types.split(',').map(str::trim) {
        if super::ctx::CARD_TYPES.contains(&word) {
            card_types.push(word);
        } else if remove_lands && BASIC_LAND_TYPES.contains(&word) {
            lands.push(word);
        } else if word == "Legendary" {
            supertypes.push(word);
        } else if ctx.world.subtypes.contains(word) {
            subtypes.push(word);
        } else {
            return unmapped(format!("key:Animate.Types={word}"));
        }
    }
    if remove_cards && card_types.is_empty() {
        return unmapped("key:Animate.RemoveCardTypes");
    }
    if remove_lands {
        mods.push(json!({"SetLandTypes": lands}));
    }
    if p.get("RemoveCreatureTypes") == Some("True") {
        mods.push(json!({"SetCreatureTypes": subtypes}));
    } else {
        mods.extend(subtypes.iter().map(|name| json!({"AddSubtype": name})));
    }
    if remove_cards {
        mods.push(json!({"SetTypes": card_types}));
    } else {
        mods.extend(card_types.iter().map(|name| json!({"AddType": name})));
    }
    mods.extend(supertypes.iter().map(|name| json!({"AddSupertype": name})));
    Ok(mods)
}

fn put_counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "PutCounter",
        &[
            "CounterType",
            "CounterNum",
            "Defined",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "DividedAsYouChoose",
        ],
    )?;
    let Some(kind) = p.get("CounterType").and_then(kind_word) else {
        return unmapped("key:PutCounter.CounterType");
    };
    let on = subject(ctx, p, "Self", None)?;
    let count = if super::effects::divided(p, "CounterNum")? {
        json!("Divided")
    } else {
        amount(ctx, p.get("CounterNum").unwrap_or("1"))?
    };
    Ok(json!({"PutCounters": {"on": on, "counter": kind, "count": count}}))
}

fn remove_counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "RemoveCounter",
        &[
            "CounterType",
            "CounterNum",
            "Defined",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
        ],
    )?;
    let from = subject(ctx, p, "Self", None)?;
    if p.get("CounterType") == Some("ALL") && matches!(p.get("CounterNum"), None | Some("All")) {
        return Ok(json!({"RemoveAllCounters": {"from": from}}));
    }
    let Some(kind) = p.get("CounterType").and_then(kind_word) else {
        return unmapped("key:RemoveCounter.CounterType");
    };
    let count = amount(ctx, p.get("CounterNum").unwrap_or("1"))?;
    Ok(json!({"RemoveCounters": {"from": from, "counter": kind, "count": count}}))
}

/// The counter a `CounterType$` word names: the upper-case Forge names, a keyword counter written as the keyword
/// (`Flying`, `First Strike`), or the experience counter of a player.
fn kind_word(word: &str) -> Option<Value> {
    const KEYWORDS: [&str; 12] = [
        "Flying",
        "Indestructible",
        "Deathtouch",
        "Trample",
        "Menace",
        "Lifelink",
        "Vigilance",
        "First Strike",
        "Double Strike",
        "Shadow",
        "Reach",
        "Hexproof",
    ];
    let word = word.trim();
    counter_kind(word).or_else(|| {
        (KEYWORDS.contains(&word) || word == "Haste" || word == "Experience")
            .then(|| json!({"Other": word.to_lowercase()}))
    })
}

fn sacrifice(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Sacrifice",
        &[
            "Defined",
            "SacValid",
            "Amount",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "RememberSacrificed",
            "Optional",
            "SacMessage",
        ],
    )?;
    // The prompt text of the choice.
    let _ = p.get("SacMessage");
    let optional = p.get("Optional") == Some("True");
    if optional && p.get("Amount").is_some_and(|n| n != "1") {
        return unmapped("key:Sacrifice.Optional-amount");
    }
    if let Some(valid) = p.get("ValidTgts")
        && !matches!(valid, "Player" | "Opponent" | "Player.Opponent")
    {
        return unmapped("key:Sacrifice.ValidTgts-objects");
    }
    let who = subject(ctx, p, "You", None)?;
    let filter = match p.get("SacValid").unwrap_or("Self") {
        "Self" => vec![json!("This")],
        valid => selector_any(ctx, valid)?,
    };
    let count = amount(ctx, p.get("Amount").unwrap_or("1"))?;
    let mut effect = json!({"Sacrifice": {"who": who, "filter": filter, "count": count}});
    if remember::wanted(p, &["RememberSacrificed"]) {
        remember::bind_moved(ctx, &mut effect, "Sacrifice", "sacrificed");
    }
    if optional {
        effect = json!({"May": {"effects": [effect]}});
    }
    Ok(effect)
}

fn fight(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Fight",
        &["Defined", "ValidTgts", "TargetMin", "TargetMax"],
    )?;
    if !p.has("ValidTgts") {
        return unmapped("key:Fight.shape");
    }
    // The fighter is named before the second creature is chosen: the card itself or the earlier target.
    let what = defined(ctx, p.get("Defined").unwrap_or_default())?;
    let with = subject(ctx, p, "", None)?;
    Ok(json!({"Fight": {"what": what, "with": with}}))
}

fn gain_control(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "GainControl",
        &[
            "ValidTgts",
            "Defined",
            "LoseControl",
            "TargetMin",
            "TargetMax",
            "NewController",
            "Untap",
            "AddKWs",
        ],
    )?;
    let duration = match p.get("LoseControl") {
        None => "Forever",
        Some("EOT") => "UntilEndOfTurn",
        Some(other) => return unmapped(format!("key:GainControl.LoseControl={other}")),
    };
    let affected = objects(subject(ctx, p, "", None)?)?;
    let controller = match p.get("NewController") {
        None => json!("You"),
        Some(word) => match defined(ctx, word)? {
            // One player takes control; a group has no single new controller.
            group if matches!(group.as_str(), Some("Opponents" | "EachPlayer")) => {
                return unmapped(format!("key:GainControl.NewController={word}"));
            }
            single => single,
        },
    };
    let mut effects = vec![json!({"Continuous": {
        "affected": affected,
        "mods": [{"SetController": controller}],
        "duration": duration,
    }})];
    if p.get("Untap") == Some("True") {
        effects.push(json!({"Untap": {"what": affected}}));
    }
    if let Some(keywords) = p.get("AddKWs") {
        let mods: Vec<Value> = keyword_list(ctx, keywords)?
            .into_iter()
            .map(|keyword| json!({ "AddKeyword": keyword }))
            .collect();
        effects.push(json!({"Continuous": {
            "affected": affected, "mods": mods, "duration": duration,
        }}));
    }
    Ok(effects)
}

/// `Attach`: the attachment (`Object$`, the card itself by default) goes onto the host (the target, or `Defined$`). An
/// `Object$ Targeted` with a `Defined$` host attaches the target to that host.
pub fn attach(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Attach",
        &[
            "ValidTgts",
            "Defined",
            "TargetMin",
            "TargetMax",
            "Object",
            "Optional",
        ],
    )?;
    let object = p.get("Object");
    if object.is_some_and(|word| word.starts_with("Valid ")) {
        return unmapped("key:Attach.Object-all");
    }
    let (what, to) = match (object, p.has("ValidTgts"), p.get("Defined")) {
        (None, true, None) => (json!("This"), subject(ctx, p, "", None)?),
        (None, false, Some(host)) => (json!("This"), defined(ctx, host)?),
        (Some(word), true, None) if !matches!(word, "Targeted" | "ThisTargetedCard") => {
            let to = subject(ctx, p, "", None)?;
            (defined(ctx, word)?, to)
        }
        (Some("Targeted" | "ThisTargetedCard"), true, Some(host)) => {
            let what = subject(ctx, p, "", None)?;
            (what, defined(ctx, host)?)
        }
        (Some(word), false, Some(host)) => (defined(ctx, word)?, defined(ctx, host)?),
        (None, false, None) => return unmapped("key:Attach.no-ValidTgts"),
        _ => return unmapped("key:Attach.Object-shape"),
    };
    let mut effect = json!({"Attach": {"what": what, "to": to}});
    if p.get("Optional") == Some("True") {
        effect = json!({"May": {"effects": [effect]}});
    }
    Ok(vec![effect])
}
