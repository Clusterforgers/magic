//! Effects on objects: destroy, counter, pump, counters, tap, sacrifice, fight and gain control.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, only, unmapped};
use super::frame::{self, Slot, Verb};
use super::keyword::{keyword_list, keyword_value};
use super::remember;
use super::valid::{defined, objects, selector_any, selector_in, subject};
use crate::forge::script::Params;

/// The effects of a verb on objects.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    let one = |value: Tr<Value>| value.map(|value| vec![value]);
    match api {
        "Destroy" => one(destroy(ctx, p)),
        "DestroyAll" => one(destroy_all(ctx, p)),
        "DamageAll" => damage_all(ctx, p),
        "DamageResolve" => damage_resolve(p),
        "PutCounterAll" => one(put_counter_all(ctx, p)),
        "Counter" => counter(ctx, p),
        "PreventDamage" => one(prevent_damage(ctx, p)),
        "Pump" => pump(ctx, p),
        "Debuff" => debuff(ctx, p),
        "AlterAttribute" => alter_attribute(ctx, p),
        "Protection" | "ProtectionAll" => protection(ctx, p, api),
        "PumpAll" => pump_all(ctx, p),
        "Animate" | "AnimateAll" => animate(ctx, p),
        "PutCounter" => put_counter(ctx, p),
        "RemoveCounter" => one(remove_counter(ctx, p)),
        "Tap" => tap(ctx, p),
        "Untap" if p.get("UntapUpTo") == Some("True") => untap_up_to(ctx, p),
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
        // A thing about its host ends with the host: once it leaves it is a new object (CR 400.7), so nothing is left to apply to.
        None | Some("EOT" | "EndOfTurn" | "UntilHostLeavesPlayOrEOT") => Ok("UntilEndOfTurn"),
        Some("Permanent") => Ok("Forever"),
        Some("UntilEndOfCombat") => Ok("UntilEndOfCombat"),
        Some("UntilYourNextTurn") => Ok("UntilYourNextTurn"),
        Some("UntilTheEndOfYourNextTurn") => Ok("UntilEndOfYourNextTurn"),
        Some("UntilHostLeavesPlay" | "AsLongAsInPlay") => Ok("WhileSourceOnBattlefield"),
        Some("AsLongAsControl") => Ok("WhileYouControlSource"),
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
        return Ok(vec![simple(ctx, p, "Tap", "Tap")?]);
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

/// `Untap` with `UntapUpTo$`: the controller picks up to `Amount$` permanents of `UntapType$` and untaps them.
fn untap_up_to(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "Untap", &["UntapUpTo", "UntapType", "Amount"])?;
    let (Some(kind), Some(count)) = (p.get("UntapType"), p.get("Amount")) else {
        return unmapped("key:Untap.UntapUpTo-without-type");
    };
    let filter = selector_any(ctx, kind)?;
    let max = amount(ctx, count)?;
    let bind = ctx.fresh("untap");
    Ok(vec![
        json!({"Choose": {"from": {"Each": filter}, "count": {"min": 0, "max": max}, "as": bind}}),
        json!({"Untap": {"what": {"Bound": bind}}}),
    ])
}

/// `TapAll`, `UntapAll`: the verb on every object the valid-string names.
fn all_of(ctx: &mut Ctx, p: &Params, verb: &str) -> Tr<Value> {
    only(p, verb, &["ValidCards", "ValidTgts"])?;
    let valid = p.get("ValidCards").unwrap_or_default();
    Ok(json!({ verb: {"what": {"Each": frame::all_matching(ctx, p, verb, valid)?}} }))
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
            "RememberLKI",
            "RememberTargets",
        ],
    )?;
    let what = subject(ctx, p, "", None)?;
    let regenerate = p.get("NoRegen") != Some("True");
    let mut effect = json!({"Destroy": {"what": what, "regenerate": regenerate}});
    if remember::wanted(p, &["RememberDestroyed", "RememberLKI", "RememberTargets"]) {
        remember::bind_moved(ctx, &mut effect, "Destroy", "destroyed");
    }
    Ok(effect)
}

fn destroy_all(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "DestroyAll",
        &["ValidCards", "ValidTgts", "NoRegen", "RememberDestroyed", "Zone"],
    )?;
    if let Some(zone) = p.get("Zone")
        && zone != "Battlefield"
    {
        return unmapped(format!("key:DestroyAll.Zone={zone}"));
    }
    let valid = p.get("ValidCards").unwrap_or_default();
    let regenerate = p.get("NoRegen") != Some("True");
    let what = json!({"Each": frame::all_matching(ctx, p, "DestroyAll", valid)?});
    let mut effect = json!({"Destroy": {"what": what, "regenerate": regenerate}});
    if remember::wanted(p, &["RememberDestroyed"]) {
        remember::bind_moved(ctx, &mut effect, "Destroy", "destroyed");
    }
    Ok(effect)
}

/// `DamageMap$ True` collects the damage of a chain of `DealDamage` and `DamageAll` lines, which `DamageResolve` then deals at
/// once. The steps of one resolution are not separated by state-based actions, so the damage dealt in order is the same.
pub fn damage_map(p: &Params) -> Tr<()> {
    match p.get("DamageMap") {
        None | Some("True") => Ok(()),
        Some(other) => unmapped(format!("key:DamageMap={other}")),
    }
}

/// The `DamageResolve` that ends a damage map: its damage was dealt by the lines before it.
fn damage_resolve(p: &Params) -> Tr<Vec<Value>> {
    only(p, "DamageResolve", &[])?;
    Ok(Vec::new())
}

fn damage_all(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "DamageAll",
        &["NumDmg", "ValidCards", "ValidPlayers", "DamageMap", "ValidTgts"],
    )?;
    if p.has("ValidTgts") && p.has("ValidPlayers") {
        return unmapped("key:DamageAll.ValidTgts-with-ValidPlayers");
    }
    damage_map(p)?;
    let amount = amount(ctx, p.get("NumDmg").unwrap_or("1"))?;
    // Players alone: no objects are hit (an empty selector would name every one).
    let mut effects = Vec::new();
    if let Some(valid) = p.get("ValidCards") {
        effects.push(
            json!({"DealDamage": {"to": {"Each": frame::all_matching(ctx, p, "DamageAll", valid)?}, "amount": amount}}),
        );
    } else if !p.has("ValidPlayers") {
        return unmapped("key:DamageAll.no-valid");
    }
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
        &["ValidCards", "CounterType", "CounterNum", "ValidTgts"],
    )?;
    let Some(kind) = p.get("CounterType").and_then(kind_word) else {
        return unmapped("key:PutCounterAll.CounterType");
    };
    let valid = p.get("ValidCards").unwrap_or_default();
    let count = amount(ctx, p.get("CounterNum").unwrap_or("1"))?;
    Ok(
        json!({"PutCounters": {"on": {"Each": frame::all_matching(ctx, p, "PutCounterAll", valid)?}, "counter": kind, "count": count}}),
    )
}

/// `TargetValidTargeting$` of a `Counter`: the spell or ability has a target the words name (`You`, `Player`, or objects on
/// the battlefield such as `Creature.YouCtrl+inRealZoneBattlefield`); several words are alternatives.
fn counter_targeting(ctx: &Ctx, words: &str) -> Tr<Value> {
    let mut alternatives = Vec::new();
    for word in words.split(',') {
        alternatives.push(match word {
            "You" => json!({"TargetsPlayer": "You"}),
            "Player" => json!({"TargetsPlayer": "EachPlayer"}),
            "Card.Self" | "Card.StrictlySelf" => json!({"Targeting": ["This"]}),
            object => {
                let spec = object
                    .replace("+inRealZoneBattlefield", "")
                    .replace(".inRealZoneBattlefield", "")
                    .replace(".inZoneBattlefield", "");
                if spec.contains("inRealZone") || spec.contains("inZone") {
                    return unmapped("key:Counter.TargetValidTargeting");
                }
                json!({"Targeting": selector_any(ctx, &spec)?})
            }
        });
    }
    Ok(match alternatives.len() {
        1 => alternatives.remove(0),
        _ => json!({"AnyOf": alternatives}),
    })
}

/// What `Counter` may target: the `TargetType$` words (`Spell`, `Activated`, `Triggered`, the abilities with a shared
/// `YouCtrl` or `YouDontCtrl`) as one selector. `ValidTgts$` filters the spells; an ability has no card type of its own, so
/// a line that has abilities admits only `Card,Emblem` (anything) or a controller (`Card.OppCtrl`).
fn counter_selector(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    let gap = || unmapped("key:Counter.TargetType");
    let Some(words) = p.get("TargetType") else {
        return gap();
    };
    let mut kinds: Vec<Value> = Vec::new();
    let mut control: Option<Option<&str>> = None;
    for word in words.split(',') {
        let (base, suffix) = match word.split_once('.') {
            Some((base, suffix)) => (base, Some(suffix)),
            None => (word, None),
        };
        let kind = match base {
            "Spell" if suffix.is_none() => json!("Spell"),
            "Activated" => json!({"Ability": "Activated"}),
            "Triggered" => json!({"Ability": "Triggered"}),
            _ => return gap(),
        };
        if base != "Spell" {
            if !matches!(suffix, None | Some("YouCtrl" | "YouDontCtrl")) {
                return gap();
            }
            if control.is_some_and(|seen| seen != suffix) {
                return gap();
            }
            control = Some(suffix);
        }
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    let spells = kinds.contains(&json!("Spell"));
    let abilities = kinds.len() > usize::from(spells);
    if spells && abilities && control.flatten().is_some() {
        return gap();
    }
    let valid = p.get("ValidTgts").unwrap_or("Card");
    let mut selector = if kinds.len() == 1 {
        kinds
    } else {
        vec![json!({"AnyOf": kinds})]
    };
    let any_card = matches!(valid, "Card" | "Card,Emblem" | "Emblem,Card");
    if abilities && !any_card && !valid.starts_with("Card.") {
        return unmapped("key:Counter.ValidTgts-of-an-ability");
    }
    if !(abilities && any_card) {
        selector.extend(selector_any(ctx, valid)?);
    }
    if let Some(word) = control.flatten() {
        selector.extend(selector_any(ctx, &format!("Card.{word}"))?);
    }
    if let Some(words) = p.get("TargetValidTargeting") {
        selector.push(counter_targeting(ctx, words)?);
    }
    Ok(selector)
}

fn counter(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Counter",
        &[
            "TargetType",
            "ValidTgts",
            "Destination",
            "Defined",
            "TargetValidTargeting",
        ],
    )?;
    // A countered spell goes to its owner's graveyard; exile instead is a shield on the spell, set before it is countered. It
    // applies to a counter that works only: a spell that can't be countered resolves as it does.
    let exiled = match p.get("Destination") {
        None | Some("Graveyard") => false,
        Some("Exile") => true,
        Some(zone) => return unmapped(format!("key:Counter.Destination={zone}")),
    };
    // The spell a cast trigger is about; no other `Defined$` is known.
    if let Some(word) = p.get("Defined") {
        let cast_trigger = ctx
            .trigger
            .as_ref()
            .is_some_and(|shape| shape.mode == "SpellCast");
        return match word {
            "TriggeredSpellAbility" | "TriggeredCard" if cast_trigger && !p.has("ValidTgts") => {
                Ok(json!({"Counter": {"what": "Triggering"}}))
            }
            _ => unmapped("key:Counter.Defined"),
        }
        .and_then(|node| {
            if exiled {
                unmapped("key:Counter.Destination=Exile-of-a-trigger")
            } else {
                Ok(vec![node])
            }
        });
    }
    let selector = counter_selector(ctx, p)?;
    let what = ctx.target(json!({"what": {"Object": selector}}))?;
    let mut effects = Vec::new();
    if exiled {
        effects.push(json!({"Shield": {
            "when": {"Countered": {"what": [{"Is": what}, "Spell"]}},
            "by": {"MoveTo": "Exile"}, "duration": "UntilEndOfTurn"}}));
    }
    effects.push(json!({"Counter": {"what": what}}));
    Ok(effects)
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

/// A pump amount: `Double` adds the object's own value as the effect begins, so it needs one object to read.
fn pump_amount(ctx: &Ctx, text: &str, single: Option<&Value>, stat: &str) -> Tr<Value> {
    if text != "Double" {
        return amount(ctx, text);
    }
    match single {
        Some(of) if of == "This" || of.get("Target").is_some() => {
            Ok(json!({"Stat": {"of": of, "stat": stat}}))
        }
        _ => unmapped("amount:Double-of-several"),
    }
}

/// The modifications of a pump line: power and toughness, then keywords.
fn pump_mods(ctx: &Ctx, p: &Params, single: Option<&Value>) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    if p.has("NumAtt") || p.has("NumDef") {
        let power = pump_amount(ctx, p.get("NumAtt").unwrap_or("0"), single, "Power")?;
        let toughness = pump_amount(ctx, p.get("NumDef").unwrap_or("0"), single, "Toughness")?;
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
            "KWChoice",
            "Defined",
            "ValidTgts",
            "Duration",
            "IsCurse",
            "TargetMin",
            "TargetMax",
            "RememberObjects",
            "PumpZone",
            "AtEOT",
            "ReplaceDyingDefined",
        ],
    )?;
    let duration = pump_duration(p)?;
    let affected = objects(subject(ctx, p, "Self", None)?)?;
    remember_objects(ctx, p)?;
    // A pump that only chooses a target ("target creature you control deals damage ... to target creature") or only
    // remembers objects changes nothing itself; the part after it uses them.
    let selects_only = !(p.has("NumAtt") || p.has("NumDef") || p.has("KW") || p.has("KWChoice"))
        && ((p.has("ValidTgts") && p.has("SubAbility")) || p.has("RememberObjects"));
    // The pumped object is on the battlefield; a pump of a card in another zone, or of a spell, is only a target choice.
    match p.get("PumpZone") {
        None | Some("Battlefield") => {}
        Some("Stack") if selects_only => {}
        Some(zone) => return unmapped(format!("key:Pump.PumpZone={zone}")),
    }
    if selects_only {
        return Ok(Vec::new());
    }
    let mut effects = Vec::new();
    if p.has("NumAtt") || p.has("NumDef") || p.has("KW") {
        let mods = pump_mods(ctx, p, Some(&affected))?;
        effects.push(
            json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}),
        );
    } else if !p.has("KWChoice") {
        return unmapped("key:Pump.nothing");
    }
    if let Some(choice) = p.get("KWChoice") {
        effects.push(keyword_choice(ctx, choice, &affected, duration)?);
    }
    if let Some(word) = p.get("AtEOT") {
        effects.push(pump_at_eot(ctx, word, &affected)?);
    }
    effects.extend(dies_shield(ctx, p)?);
    Ok(effects)
}

/// `KWChoice$`: the controller picks one keyword of the list as the effect resolves and the object gains it. A group of
/// objects stays a gap: Forge asks once for the group, which this node does not say.
fn keyword_choice(ctx: &mut Ctx, choice: &str, affected: &Value, duration: &str) -> Tr<Value> {
    if affected.get("Each").is_some() {
        return unmapped("key:Pump.KWChoice-for-a-group");
    }
    let mut options = Vec::new();
    for word in choice.split(',').map(str::trim) {
        let keyword = keyword_value(ctx, word)?;
        options.push(json!({"Continuous": {
            "affected": affected, "mods": [{"AddKeyword": keyword}], "duration": duration}}));
    }
    either(options)
}

/// The choice of one of several effects, written as nested `Either`.
pub fn either(mut options: Vec<Value>) -> Tr<Value> {
    let Some(mut rest) = options.pop() else {
        return unmapped("key:Choice.no-options");
    };
    while let Some(option) = options.pop() {
        rest = json!({"Either": {"first": [option], "second": [rest]}});
    }
    Ok(rest)
}

/// `AtEOT$` of a pump: what happens to the pumped object at the next end step. The object is the source itself or a
/// target, which the delayed trigger holds by a name.
fn pump_at_eot(ctx: &mut Ctx, word: &str, affected: &Value) -> Tr<Value> {
    if affected == &json!("This") || affected.get("Bound").is_some() {
        return super::tokens_copy::at_eot(word, affected);
    }
    if affected.get("Target").is_none() {
        return unmapped("key:Pump.AtEOT-of-a-group");
    }
    let name = ctx.fresh("pumped");
    let inner = super::tokens_copy::at_eot(word, &json!({ "Bound": name }))?;
    Ok(json!({"RepeatEach": {"over": affected, "as": name, "effects": [inner]}}))
}

/// `RememberObjects$` of a `Pump`: what the line names is what `Remembered` stands for in the lines after it.
fn remember_objects(ctx: &mut Ctx, p: &Params) -> Tr<()> {
    let Some(word) = p.get("RememberObjects") else {
        return Ok(());
    };
    let what = match word {
        "ThisTargetedCard" if p.has("ValidTgts") => ctx.last_target()?,
        "Targeted" | "TargetedController" | "TargetedOwner" | "ParentTarget" | "Self" => {
            defined(ctx, word)?
        }
        word if super::triggered::is_word(word) => defined(ctx, word)?,
        other => return unmapped(format!("key:Pump.RememberObjects={other}")),
    };
    ctx.remembered = Some(what);
    Ok(())
}

fn pump_all(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "PumpAll",
        &[
            "ValidTgts",
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
    let affected = json!({"Each": group_of_player(ctx, p, valid)?});
    let mods = pump_mods(ctx, p, None)?;
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
            "staticAbilities",
            "RemoveCreatureTypes",
            "RemoveCardTypes",
            "RemoveLandTypes",
            "AddAllCreatureTypes",
            "ValidCards",
            "Duration",
            "TargetMin",
            "TargetMax",
            "AtEOT",
        ],
    )?;
    let duration = duration(p.get("Duration"))?;
    // `AnimateAll` names the objects by a valid-string.
    let affected = match p.get("ValidCards") {
        Some(valid) => json!({"Each": selector_any(ctx, valid)?}),
        None => objects(subject(ctx, p, "Self", None)?)?,
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
        if colors == "ChosenColor" {
            match (ctx.chosen_kind, ctx.chosen.as_ref()) {
                (Some("Color"), Some(name)) => mods.push(json!({"SetColorsChosen": name})),
                _ => return unmapped("key:Animate.Colors=ChosenColor"),
            }
        } else {
            mods.push(json!({"SetColors": color_names(colors)?}));
        }
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
    let mut effects =
        vec![json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}})];
    if let Some(word) = p.get("AtEOT") {
        if p.has("ValidCards") {
            return unmapped("key:Animate.AtEOT-of-a-group");
        }
        effects.push(pump_at_eot(ctx, word, &affected)?);
    }
    Ok(effects)
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
    let mut chosen_mods = Vec::new();
    for word in types.split(',').map(str::trim) {
        if word == "ChosenType" {
            let by_kind = match (ctx.chosen_kind, ctx.chosen.as_ref()) {
                (Some("LandType"), Some(name)) if remove_lands => ("SetLandTypesChosen", name),
                (Some("CreatureType"), Some(name))
                    if p.get("RemoveCreatureTypes") == Some("True") =>
                {
                    ("SetCreatureTypesChosen", name)
                }
                _ => return unmapped("key:Animate.Types=ChosenType-kind"),
            };
            chosen_mods.push(json!({ by_kind.0: by_kind.1 }));
            continue;
        }
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
    if !chosen_mods.is_empty() {
        if !lands.is_empty() || !subtypes.is_empty() || remove_cards {
            return unmapped("key:Animate.Types=ChosenType-with-more");
        }
        mods.extend(chosen_mods);
        mods.extend(supertypes.iter().map(|name| json!({"AddSupertype": name})));
        return Ok(mods);
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

/// `Monstrosity$`, `Adapt$` and `Bolster$` on a `PutCounter` line: the keyword action with its number.
fn counter_action(ctx: &mut Ctx, p: &Params) -> Option<Tr<Vec<Value>>> {
    let key = ["Monstrosity", "Adapt", "Bolster"]
        .into_iter()
        .find(|key| p.has(key))?;
    Some(
        only(p, "PutCounter", &[key])
            .and_then(|()| amount(ctx, p.get(key).unwrap_or("1")))
            .map(|count| vec![json!({ key: {"count": count} })]),
    )
}

fn put_counter(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    if let Some(done) = counter_action(ctx, p) {
        return done;
    }
    if let Some(n) = p.get("Support") {
        // Support N (CR 701.41): a +1/+1 counter on each of up to N other target creatures.
        only(p, "PutCounter", &["Support"])?;
        let Ok(n) = n.parse::<u32>() else {
            return unmapped("key:PutCounter.Support");
        };
        let line = format!(
            "ValidTgts$ Creature.Other | TargetMin$ 0 | TargetMax$ {n} | CounterType$ P1P1 | CounterNum$ 1"
        );
        return put_counter(ctx, &Params::parse(&line));
    }
    only(
        p,
        "PutCounter",
        &[
            "CounterType",
            "CounterNum",
            "Defined",
            "Choices",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "DividedAsYouChoose",
        ],
    )?;
    let kinds = counter_choices(p.get("CounterType").unwrap_or_default())?;
    let mut effects = Vec::new();
    // `Choices$` is a pick made as the effect resolves, from the objects that match, with no targets.
    let on = match p.get("Choices") {
        Some(_) if p.has("ValidTgts") || p.has("Defined") => {
            return unmapped("key:PutCounter.Choices-with-a-target");
        }
        Some(valid) => {
            let from = json!({"Each": selector_in(ctx, valid, None)?});
            let bind = ctx.fresh("card");
            effects.push(json!({"Choose": {
                "who": "You", "from": from, "count": {"min": 1, "max": 1}, "as": bind}}));
            json!({ "Bound": bind })
        }
        None => subject(ctx, p, "Self", None)?,
    };
    let count = if super::effects::divided(p, "CounterNum")? {
        json!("Divided")
    } else {
        amount(ctx, p.get("CounterNum").unwrap_or("1"))?
    };
    let puts = kinds
        .into_iter()
        .map(|kind| json!({"PutCounters": {"on": on, "counter": kind, "count": count}}))
        .collect();
    effects.push(either(puts)?);
    Ok(effects)
}

/// The kinds a `CounterType$` lists: one is the counter put, several are the controller's choice of one.
fn counter_choices(words: &str) -> Tr<Vec<Value>> {
    words
        .split(',')
        .map(|word| match kind_word(word) {
            Some(kind) => Ok(kind),
            None => unmapped("key:PutCounter.CounterType"),
        })
        .collect()
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
    // Every counter of the kind: as many as the object holds as the step resolves.
    let count = match p.get("CounterNum") {
        Some("All") => json!({"Counters": {"on": from, "counter": kind}}),
        other => amount(ctx, other.unwrap_or("1"))?,
    };
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
    // Counters that cards name in words; any other word stays a gap.
    const NAMED: [&str; 4] = ["Quest", "Exalted", "Decayed", "Verse"];
    let word = word.trim();
    counter_kind(word).or_else(|| {
        (KEYWORDS.contains(&word)
            || NAMED.contains(&word)
            || word == "Haste"
            || word == "Experience")
            .then(|| json!({"Other": word.to_lowercase()}))
    })
}

/// `Sacrifice`: the player `Defined$` names sacrifices permanents.
const SACRIFICE: Verb = Verb {
    label: "Sacrifice",
    slots: &[
        Slot::Actor,
        Slot::Pool,
        Slot::Target,
        Slot::Limits,
        Slot::Count,
        Slot::Optional,
    ],
    actor: "Defined",
    pool: "SacValid",
    count: "Amount",
    own: &["RememberSacrificed", "SacMessage"],
};

fn sacrifice(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    SACRIFICE.admit(p)?;
    // The prompt text of the choice.
    let _ = p.get("SacMessage");
    let may = match p.get("Optional") {
        None | Some("False") => false,
        Some("True") => true,
        Some(_) => return unmapped("key:Sacrifice.Optional"),
    };
    if may && p.get("Amount").is_some_and(|n| n != "1") {
        return unmapped("key:Sacrifice.Optional-amount");
    }
    let who = frame::player_actor(ctx, p, &SACRIFICE)?;
    let filter = match p.get("SacValid") {
        None | Some("Self") if who == json!("You") => vec![json!("This")],
        None | Some("Self") => return unmapped("key:Sacrifice.SacValid"),
        Some(valid) => selector_any(ctx, valid)?,
    };
    let count = frame::count(ctx, p, &SACRIFICE)?.max;
    let mut effect = json!({"Sacrifice": {"who": who, "filter": filter, "count": count}});
    if remember::wanted(p, &["RememberSacrificed"]) {
        remember::bind_moved(ctx, &mut effect, "Sacrifice", "sacrificed");
    }
    if may {
        effect = super::frame::ask(vec![effect], &json!("You")).remove(0);
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
            "AllValid",
        ],
    )?;
    let duration = match p.get("LoseControl") {
        None => "Forever",
        Some("EOT") => "UntilEndOfTurn",
        Some(other) => return unmapped(format!("key:GainControl.LoseControl={other}")),
    };
    let affected = match p.get("AllValid") {
        Some(valid) => json!({"Each": group_of_player(ctx, p, valid)?}),
        None => objects(subject(ctx, p, "", None)?)?,
    };
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
        effect = super::frame::ask(vec![effect], &json!("You")).remove(0);
    }
    Ok(vec![effect])
}

/// `Debuff`: the object loses the keywords for a while ("target creature loses flying until end of turn").
fn debuff(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Debuff",
        &[
            "Keywords",
            "Defined",
            "ValidTgts",
            "Duration",
            "TargetMin",
            "TargetMax",
        ],
    )?;
    let Some(keywords) = p.get("Keywords") else {
        return unmapped("key:Debuff.nothing");
    };
    let duration = duration(p.get("Duration"))?;
    let affected = objects(subject(ctx, p, "Self", None)?)?;
    let mods: Vec<Value> = keyword_list(ctx, keywords)?
        .into_iter()
        .map(|keyword| json!({ "RemoveKeyword": keyword }))
        .collect();
    Ok(vec![
        json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}),
    ])
}

/// `Protection` and `ProtectionAll`: the objects gain protection from the colour or card type their controller chooses as
/// the effect begins, or from the colours it names, for a while.
fn protection(ctx: &mut Ctx, p: &Params, api: &str) -> Tr<Vec<Value>> {
    only(
        p,
        api,
        &[
            "Gains",
            "Choices",
            "Choser",
            "Duration",
            "Defined",
            "ValidTgts",
            "ValidCards",
            "TargetMin",
            "TargetMax",
        ],
    )?;
    if !matches!(p.get("Choser"), None | Some("Controller")) {
        return unmapped("key:Protection.Choser");
    }
    let duration = duration(p.get("Duration"))?;
    let affected = if api == "ProtectionAll" {
        let valid = p.get("ValidCards").unwrap_or_default();
        json!({"Each": selector_any(ctx, valid)?})
    } else {
        objects(subject(ctx, p, "Self", None)?)?
    };
    let mods = match (p.get("Gains"), p.get("Choices")) {
        (Some("Choice"), Some("AnyColor")) => vec![json!("AddProtectionFromChosenColor")],
        (Some("Choice"), Some("CardType")) => vec![json!("AddProtectionFromChosenCardType")],
        (Some("Choice"), choices) => {
            return unmapped(format!("key:Protection.Choices={}", choices.unwrap_or("")));
        }
        (Some(gains), None) => {
            let mut mods = Vec::new();
            for word in gains.split(',').map(str::trim) {
                let color = ["white", "blue", "black", "red", "green"].contains(&word);
                if !color {
                    return unmapped(format!("key:Protection.Gains={gains}"));
                }
                let capital = word[..1].to_uppercase() + &word[1..];
                let keyword = keyword_value(ctx, &format!("Protection:Card.{capital}"))?;
                mods.push(json!({ "AddKeyword": keyword }));
            }
            mods
        }
        _ => return unmapped("key:Protection.Gains"),
    };
    Ok(vec![
        json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}),
    ])
}

/// `AlterAttribute`: a prepare card becomes prepared. The other attributes (suspected, solved, plotted, saddled) and the
/// removal of one (`Activate$ False`) stay gaps named after the value.
fn alter_attribute(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "AlterAttribute",
        &["Attributes", "Defined", "ValidTgts", "Activate"],
    )?;
    match p.get("Attributes") {
        Some("Prepared") => {}
        other => {
            return unmapped(format!(
                "key:AlterAttribute.Attributes={}",
                other.unwrap_or_default()
            ));
        }
    }
    if !matches!(p.get("Activate"), None | Some("True")) {
        return unmapped("key:AlterAttribute.Activate");
    }
    let what = subject(ctx, p, "Self", None)?;
    Ok(vec![json!({"Prepare": {"what": what}})])
}

/// The objects a group line (`PumpAll`, `GainControl` with `AllValid$`) acts on. A `ValidTgts$` that names a player is the
/// player whose objects they are ("creatures target player controls"): the target is declared first, and a valid-string that
/// does not name the player itself is limited to the objects that player controls.
fn group_of_player(ctx: &mut Ctx, p: &Params, valid: &str) -> Tr<Vec<Value>> {
    if !p.has("ValidTgts") {
        return selector_any(ctx, valid);
    }
    if !matches!(p.get("ValidTgts"), Some("Player" | "Opponent")) {
        return unmapped("key:Group.ValidTgts-not-a-player");
    }
    let target = subject(ctx, p, "", None)?;
    let mut preds = selector_any(ctx, valid)?;
    // A valid-string that says whose objects they are ("you control") already did.
    let owned = preds
        .iter()
        .any(|pred| pred.get("ControlledBy").is_some() || pred.get("OwnedBy").is_some());
    if !valid.contains("TargetedPlayer") && !owned {
        preds.push(json!({"ControlledBy": target}));
    }
    Ok(preds)
}

/// `ReplaceDyingDefined$` on a damage or pump line ("if that creature would die this turn, exile it instead"): after the
/// effect, a shield sends the creature the line targets to exile if it would die. Other objects stay a gap by name.
pub fn with_dies_shield(ctx: &mut Ctx, p: &Params, node: Value) -> Tr<Vec<Value>> {
    let mut effects = vec![node];
    effects.extend(dies_shield(ctx, p)?);
    Ok(effects)
}

/// The shield alone, when the line has the key.
fn dies_shield(ctx: &mut Ctx, p: &Params) -> Tr<Option<Value>> {
    let Some(word) = p.get("ReplaceDyingDefined") else {
        return Ok(None);
    };
    let kind = match word {
        "Targeted" | "ThisTargetedCard" | "ThisTargetedCard.Creature" if p.has("ValidTgts") => {
            ctx.last_target()?
        }
        _ => return unmapped(format!("key:ReplaceDyingDefined={word}")),
    };
    let shield = json!({"Shield": {
        "when": {"ZoneChange": {
            "what": [{"Is": kind}, {"Type": "Creature"}], "from": "Battlefield", "to": "Graveyard"}},
        "by": {"MoveTo": "Exile"},
        "duration": "UntilEndOfTurn"}});
    Ok(Some(shield))
}
