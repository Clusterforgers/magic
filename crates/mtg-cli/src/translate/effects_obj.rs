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
        "DamageResolve" => damage_resolve(ctx, p),
        "PutCounterAll" => one(put_counter_all(ctx, p)),
        "RemoveCounterAll" => one(remove_counter_all(ctx, p)),
        "AddOrRemoveCounter" => one(add_or_remove_counter(ctx, p)),
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
        "Airbend" => airbend(ctx, p),
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
        Some("UntilYourNextEndStep") => Ok("UntilYourNextEndStep"),
        Some("UntilTheEndOfYourNextTurn") => Ok("UntilEndOfYourNextTurn"),
        Some("UntilHostLeavesPlay" | "AsLongAsInPlay") => Ok("WhileSourceOnBattlefield"),
        Some("AsLongAsControl") => Ok("WhileYouControlSource"),
        Some("UntilUntaps") => Ok("WhileSourceTapped"),
        Some(other) => unmapped(format!("duration:{other}")),
    }
}

/// `Airbend`: exile the objects, which their owners may cast for {2} rather than their mana costs while they stay exiled.
fn airbend(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "Airbend",
        &["ValidTgts", "Defined", "TargetMin", "TargetMax"],
    )?;
    // A spell on the stack is exiled from the stack, which no effect of the engine does for an airbend yet.
    if p.get("ValidTgts")
        .is_some_and(|valid| valid.contains("inZoneStack"))
        || p.get("TgtZone").is_some()
    {
        return unmapped("key:Airbend.TgtZone");
    }
    let what = subject(ctx, p, "", None)?;
    Ok(vec![
        json!({"Exile": {"what": what, "as": "airbent"}}),
        json!({"Rule": {"rule": {"MayCastRather": {"only": {"Bound": "airbent"}, "cost": "{2}"}}, "duration": "Forever"}}),
    ])
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
/// `RememberTapped$` of a `TapAll` binds the permanents that became tapped.
fn all_of(ctx: &mut Ctx, p: &Params, verb: &str) -> Tr<Value> {
    if verb == "Tap" {
        only(p, verb, &["ValidCards", "ValidTgts", "RememberTapped"])?;
    } else {
        only(p, verb, &["ValidCards", "ValidTgts"])?;
    }
    let valid = p.get("ValidCards").unwrap_or_default();
    let mut effect = json!({ verb: {"what": {"Each": frame::all_matching(ctx, p, verb, valid)?}} });
    if remember::wanted(p, &["RememberTapped"]) {
        remember::bind_moved(ctx, &mut effect, verb, "tapped");
    }
    Ok(effect)
}

/// The indices of the last targets of the ability when a line that declares none reads `Defined$ Targeted` and each of
/// them was declared by a line that only chooses it; empty otherwise.
fn r6e_chosen(ctx: &Ctx, p: &Params) -> Vec<u8> {
    let remembers = [
        "RememberDestroyed",
        "RememberLKI",
        "RememberTargets",
        "AlwaysRemember",
    ];
    // Looked at without marking the keys read: the line reads them itself when this is not its shape.
    let peek = |key: &str| p.iter().find(|(k, _)| *k == key).map(|(_, v)| v);
    if peek("Defined") != Some("Targeted")
        || peek("ValidTgts").is_some()
        || remembers.iter().any(|key| peek(key).is_some())
    {
        return Vec::new();
    }
    let mut chosen = Vec::new();
    for (index, spec) in ctx.targets.iter().enumerate().rev() {
        let Ok(index) = u8::try_from(index) else {
            break;
        };
        if !ctx.r6e_selected.contains(&(index, spec.clone())) {
            break;
        }
        chosen.push(index);
    }
    chosen.reverse();
    if chosen.len() >= 2 {
        p.get("Defined");
    }
    chosen
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
            "AlwaysRemember",
        ],
    )?;
    let regenerate = p.get("NoRegen") != Some("True");
    let chosen = r6e_chosen(ctx, p);
    if chosen.len() >= 2 {
        // "Destroy target creature and target land": each target the lines before only chose, together.
        let each: Vec<Value> = chosen
            .iter()
            .map(|index| json!({"Destroy": {"what": {"Target": index}, "regenerate": regenerate}}))
            .collect();
        return Ok(json!({"Together": {"effects": each}}));
    }
    let what = subject(ctx, p, "", None)?;
    let mut effect = json!({"Destroy": {"what": what, "regenerate": regenerate}});
    // The target is remembered whether or not it is destroyed (a regenerated creature is still "that creature").
    let remembers = remember::wanted(p, &["RememberDestroyed", "RememberLKI", "RememberTargets"]);
    if p.get("AlwaysRemember") == Some("True") && p.has("ValidTgts") {
        ctx.remembered = Some(what);
        return Ok(effect);
    }
    if remembers {
        remember::bind_moved(ctx, &mut effect, "Destroy", "destroyed");
    }
    Ok(effect)
}

fn destroy_all(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "DestroyAll",
        &[
            "ValidCards",
            "ValidTgts",
            "NoRegen",
            "RememberDestroyed",
            "Zone",
        ],
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

/// The `DamageResolve` that ends a damage map: its damage was dealt by the lines before it. Inside a chain it leaves a
/// mark that the line that began the chain turns into the end of its batch (`batch_damage`).
fn damage_resolve(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(p, "DamageResolve", &[])?;
    Ok(if ctx.damage_open {
        vec![json!({DAMAGE_END: true})]
    } else {
        Vec::new()
    })
}

const DAMAGE_END: &str = "~DamageResolve";

/// Wraps the steps of a damage map chain, up to its end mark, in one `Together` batch, and drops every end mark.
pub fn batch_damage(mut effects: Vec<Value>) -> Vec<Value> {
    let end = effects.iter().position(|e| e.get(DAMAGE_END).is_some());
    let tail = end.map(|at| {
        let tail = effects.split_off(at + 1);
        effects.pop();
        tail
    });
    let mut out = match tail {
        Some(_) => vec![json!({"Together": {"effects": effects}})],
        None => effects,
    };
    out.extend(tail.unwrap_or_default());
    for effect in &mut out {
        strip_end(effect);
    }
    out.retain(|e| e.get(DAMAGE_END).is_none());
    out
}

fn strip_end(value: &mut Value) {
    match value {
        Value::Array(items) => {
            items.retain(|e| e.get(DAMAGE_END).is_none());
            items.iter_mut().for_each(strip_end);
        }
        Value::Object(fields) => fields.values_mut().for_each(strip_end),
        _ => {}
    }
}

fn damage_all(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    only(
        p,
        "DamageAll",
        &[
            "NumDmg",
            "ValidCards",
            "ValidPlayers",
            "DamageMap",
            "ValidTgts",
            "DamageSource",
            "ReplaceDyingDefined",
        ],
    )?;
    if p.has("ValidTgts") && p.has("ValidPlayers") {
        return unmapped("key:DamageAll.ValidTgts-with-ValidPlayers");
    }
    damage_map(p)?;
    let amount = amount(ctx, p.get("NumDmg").unwrap_or("1"))?;
    // The creature or permanent that deals the damage, when it is not the ability's own source.
    let source = p
        .get("DamageSource")
        .map(|word| defined(ctx, word))
        .transpose()?;
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
    if let Some(source) = source {
        for effect in &mut effects {
            effect["DealDamage"]["source"] = source.clone();
        }
    }
    effects.extend(dies_shield(ctx, p)?);
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

/// `AddOrRemoveCounter`: the controller puts counters of a kind on the object or takes them off ("put a plague counter on
/// it or remove one from it"); with `Optional$` the controller may do neither.
fn add_or_remove_counter(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "AddOrRemoveCounter",
        &[
            "CounterType",
            "CounterNum",
            "Defined",
            "ValidTgts",
            "TargetMin",
            "TargetMax",
            "Optional",
            "DefinedPlayer",
        ],
    )?;
    let Some(kind) = p.get("CounterType").and_then(kind_word) else {
        return unmapped("key:AddOrRemoveCounter.CounterType");
    };
    let on = subject(ctx, p, "Self", None)?;
    let count = amount(ctx, p.get("CounterNum").unwrap_or("1"))?;
    let put = json!({"PutCounters": {"on": on, "counter": kind, "count": count}});
    let take = json!({"RemoveCounters": {"from": on, "counter": kind, "count": count}});
    let mut options = vec![json!([put]), json!([take])];
    match p.get("Optional") {
        None | Some("False") => {}
        Some("True") => options.push(json!([])),
        Some(_) => return unmapped("key:AddOrRemoveCounter.Optional"),
    }
    let mut choice = json!({"OneOf": {"options": options}});
    if let Some(word) = p.get("DefinedPlayer") {
        choice["OneOf"]["who"] = defined(ctx, word)?;
    }
    Ok(choice)
}

/// `RemoveCounterAll`: counters off every battlefield object that matches, every kind, every counter of one kind, or a
/// number of them from each.
fn remove_counter_all(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "RemoveCounterAll",
        &[
            "ValidCards",
            "CounterType",
            "CounterNum",
            "AllCounters",
            "AllCounterTypes",
            "ValidTgts",
        ],
    )?;
    let valid = p.get("ValidCards").unwrap_or_default();
    let from = json!({"Each": frame::all_matching(ctx, p, "RemoveCounterAll", valid)?});
    if p.get("AllCounterTypes") == Some("True") {
        return Ok(json!({"RemoveAllCounters": {"from": from}}));
    }
    let Some(kind) = p.get("CounterType").and_then(kind_word) else {
        return unmapped("key:RemoveCounterAll.CounterType");
    };
    // A line with no count takes every counter of the kind (Corrosion).
    let every = p.get("AllCounters") == Some("True");
    let Some(number) = p.get("CounterNum").filter(|_| !every) else {
        return Ok(json!({"RemoveAllCounters": {"from": from, "counter": kind}}));
    };
    let count = amount(ctx, number)?;
    Ok(json!({"RemoveCounters": {"from": from, "counter": kind, "count": count}}))
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
        Some(of)
            if of == "This"
                || of == "Triggering"
                || of.get("Target").is_some()
                || of.get("Bound").is_some() =>
        {
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
            if super::statics_combat::is_rule_word(word) {
                continue;
            }
            match hidden(word) {
                Some(found) => mods.extend(found),
                None => mods.push(json!({ "AddKeyword": keyword_value(ctx, word)? })),
            }
        }
    }
    mods.extend(super::statics_combat::block_capacity(p)?);
    Ok(mods)
}

/// The effects of the hidden keywords of a pump line that are rules, not changes to the object: "must be blocked if able"
/// and "prevent all damage that would be dealt to it".
fn hidden_effects(p: &Params, affected: &Value, duration: &str) -> Tr<Vec<Value>> {
    p.get("KW")
        .into_iter()
        .flat_map(|keywords| keywords.split('&').map(str::trim))
        .filter(|word| super::statics_combat::is_rule_word(word))
        .map(|word| super::statics_combat::hidden_effect(word, affected, duration))
        .collect::<Tr<Vec<Vec<Value>>>>()
        .map(|found| found.concat())
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
            "CanBlockAmount",
            "CanBlockAny",
        ],
    )?;
    let duration = pump_duration(p)?;
    let subject = subject(ctx, p, "Self", None)?;
    if let Some(found) = player_keywords(p, &subject, duration) {
        return found;
    }
    let affected = objects(subject)?;
    remember_objects(ctx, p)?;
    // A pump that only chooses a target ("target creature you control deals damage ... to target creature") or only
    // remembers objects changes nothing itself; the part after it uses them.
    let selects_only = !(p.has("NumAtt")
        || p.has("NumDef")
        || p.has("KW")
        || p.has("KWChoice")
        || p.has("CanBlockAmount")
        || p.has("CanBlockAny"))
        && ((p.has("ValidTgts") && p.has("SubAbility")) || p.has("RememberObjects"));
    // The pumped object is on the battlefield; a pump of a card in another zone, or of a spell, is only a target choice.
    match p.get("PumpZone") {
        None | Some("Battlefield") => {}
        Some("Stack") if selects_only => {}
        Some(zone) => return unmapped(format!("key:Pump.PumpZone={zone}")),
    }
    if selects_only {
        if p.has("ValidTgts")
            && let Some(spec) = ctx.targets.last()
            && let Ok(index) = u8::try_from(ctx.targets.len() - 1)
        {
            let spec = spec.clone();
            ctx.r6e_selected.push((index, spec));
        }
        return Ok(Vec::new());
    }
    let mut effects = Vec::new();
    let doubles = p.get("NumAtt") == Some("Double") || p.get("NumDef") == Some("Double");
    if doubles && affected.get("Each").is_some() {
        // Each object doubles its own value: one pass per object, the pass reading the object it is for.
        let name = ctx.fresh("doubled");
        let one = json!({ "Bound": name });
        let mods = pump_mods(ctx, p, Some(&one))?;
        let pass = json!({"Continuous": {"affected": one, "mods": mods, "duration": duration}});
        effects.push(json!({"RepeatEach": {"over": affected, "as": name, "effects": [pass]}}));
    } else if p.has("NumAtt")
        || p.has("NumDef")
        || p.has("KW")
        || p.has("CanBlockAmount")
        || p.has("CanBlockAny")
    {
        let mods = pump_mods(ctx, p, Some(&affected))?;
        let rules = hidden_effects(p, &affected, duration)?;
        if mods.is_empty() && rules.is_empty() {
            return unmapped("key:Pump.nothing");
        }
        if !mods.is_empty() {
            effects.push(
                json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}),
            );
        }
        effects.extend(rules);
    } else if !p.has("KWChoice") && !p.has("AtEOT") {
        // A line with only `AtEOT$` is the clean-up of the object ("sacrifice it at the beginning of the next end step").
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

/// "You gain hexproof until end of turn": a pump of players that only gives keywords a player can have (hexproof, shroud) is
/// a rule for the duration; `None` for a pump of objects or one that changes more.
fn player_keywords(p: &Params, subject: &Value, duration: &str) -> Option<Tr<Vec<Value>>> {
    let player = match subject {
        Value::String(word) => matches!(
            word.as_str(),
            "You" | "Opponents" | "EachPlayer" | "AttachedPlayer"
        ),
        Value::Object(map) => map.contains_key("ControllerOf") || map.contains_key("OwnerOf"),
        _ => false,
    };
    let changes_more = [
        "NumAtt",
        "NumDef",
        "KWChoice",
        "AtEOT",
        "ValidTgts",
        "PumpZone",
        "RememberObjects",
        "CanBlockAmount",
        "CanBlockAny",
        "ReplaceDyingDefined",
    ]
    .iter()
    .any(|key| p.has(key));
    if !player || changes_more {
        return None;
    }
    let keywords = p.get("KW")?;
    let mut rules = Vec::new();
    for word in keywords.split('&').map(str::trim) {
        if !matches!(word, "Hexproof" | "Shroud") {
            return Some(unmapped(format!("key:Pump.player-keyword-{word}")));
        }
        rules.push(json!({"Rule": {"rule": {"PlayersHave": {"who": subject, "keyword": word}}, "duration": duration}}));
    }
    Some(Ok(rules))
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
    let rules = hidden_effects(p, &affected, duration)?;
    if mods.is_empty() && rules.is_empty() {
        return unmapped("key:Pump.nothing");
    }
    let mut effects = Vec::new();
    if !mods.is_empty() {
        effects.push(
            json!({"Continuous": {"affected": affected, "mods": mods, "duration": duration}}),
        );
    }
    effects.extend(rules);
    Ok(effects)
}

/// How long an animation lasts. Forge ends "for as long as you control ~" when the host leaves; it also ends when another
/// player gains control of it (CR 611.2b). A card that says both that and "remains on the battlefield" stays a gap.
fn animate_duration(ctx: &Ctx, p: &Params) -> Tr<&'static str> {
    let duration = duration(p.get("Duration"))?;
    let oracle = ctx.oracle.to_lowercase();
    if duration != "WhileSourceOnBattlefield" || !oracle.contains("for as long as you control") {
        return Ok(duration);
    }
    if oracle.contains("remains on the battlefield") {
        return unmapped("key:Animate.Duration-while-you-control-or-remains");
    }
    Ok("WhileYouControlSource")
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
            "Replacements",
            "RemoveCreatureTypes",
            "RemoveCardTypes",
            "RemoveLandTypes",
            "AddAllCreatureTypes",
            "ValidCards",
            "Duration",
            "TargetMin",
            "TargetMax",
            "AtEOT",
            "HiddenKeywords",
        ],
    )?;
    // Effects change permanents only (the layers read nothing else): "target spell becomes red" would change nothing.
    // (The header folded `TgtZone$` into the valid-string as `inZoneStack` by now.)
    if [p.get("TgtZone"), p.get("ValidTgts")]
        .into_iter()
        .flatten()
        .any(|text| text.contains("Stack"))
    {
        return unmapped("key:Animate.TgtZone=Stack");
    }
    let duration = animate_duration(ctx, p)?;
    // `AnimateAll` names the objects by a valid-string.
    let affected = match p.get("ValidCards") {
        // "creatures target player controls": Forge narrows the group to the target's at run time, the selector would not.
        Some(_)
            if p.get("ValidTgts")
                .is_some_and(|valid| valid.contains("Player")) =>
        {
            return unmapped("key:AnimateAll.ValidTgts=Player");
        }
        Some(valid) => json!({"Each": selector_any(ctx, valid)?}),
        None => objects(subject(ctx, p, "Self", None)?)?,
    };
    let mut mods = Vec::new();
    match (p.get("Power"), p.get("Toughness")) {
        (Some(power), Some(toughness)) => {
            let (power, toughness) = (amount(ctx, power)?, amount(ctx, toughness)?);
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
        (Some(power), None) => mods.push(json!({"SetPower": amount(ctx, power)?})),
        (None, Some(toughness)) => mods.push(json!({"SetToughness": amount(ctx, toughness)?})),
        (None, None) => {}
    }
    if let Some(colors) = p.get("Colors") {
        // Forge adds the colors unless told to overwrite; Oracle "becomes a green creature" sets the color (CR 205.3,
        // layer 5) and only "in addition to its other colors" adds.
        let text = p
            .get("SpellDescription")
            .unwrap_or(ctx.oracle)
            .to_lowercase();
        let adds = p.get("OverwriteColors") != Some("True")
            && (text.contains("other colors") || text.contains("additional color"));
        if adds {
            if colors == "ChosenColor" {
                return unmapped("key:Animate.Colors-added");
            }
            mods.push(json!({"AddColors": color_names(colors)?}));
        } else if colors == "ChosenColor" {
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
    mods.extend(super::effects_animate::restrictions(p)?);
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

/// Colours as the DSL names them; `Colorless` names none, so setting it gives the empty set (CR 105.2c).
pub fn color_names(colors: &str) -> Tr<Vec<&'static str>> {
    colors
        .split(',')
        .filter(|word| word.trim() != "Colorless")
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
            "CounterTypes",
            "DividedAsYouChoose",
            "CounterNumPerDefined",
        ],
    )?;
    if let Some(name) = p.get("CounterNumPerDefined") {
        return per_object_counters(ctx, p, name);
    }
    // `CounterTypes$` is every kind in the list at once ("a flying counter and a lifelink counter"); `CounterType$` a list is a choice.
    let all_kinds = p.has("CounterTypes");
    if all_kinds && (p.has("CounterType") || p.has("DividedAsYouChoose")) {
        return unmapped("key:PutCounter.CounterTypes");
    }
    let kinds = counter_choices(
        p.get("CounterTypes")
            .or(p.get("CounterType"))
            .unwrap_or_default(),
    )?;
    let mut effects = Vec::new();
    // "Put a +1/+1 counter on that creature and a +1/+1 counter on this creature": one put on each, in the order named.
    if let Some((first, second)) = p.get("Defined").and_then(|word| word.split_once(" & ")) {
        let [kind] = kinds.as_slice() else {
            return unmapped("key:PutCounter.Defined-pair-with-a-choice");
        };
        if p.has("ValidTgts") || p.has("Choices") || p.has("DividedAsYouChoose") {
            return unmapped("key:PutCounter.Defined-pair");
        }
        let count = amount(ctx, p.get("CounterNum").unwrap_or("1"))?;
        for word in [first, second] {
            let on = super::valid::objects(defined(ctx, word)?)?;
            effects.push(json!({"PutCounters": {"on": on, "counter": kind, "count": count}}));
        }
        return Ok(effects);
    }
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
    let puts: Vec<Value> = kinds
        .into_iter()
        .map(|kind| json!({"PutCounters": {"on": on, "counter": kind, "count": count}}))
        .collect();
    if all_kinds {
        effects.extend(puts);
    } else {
        effects.push(either(puts)?);
    }
    Ok(effects)
}

/// `CounterNumPerDefined$ X`: "put a number of +1/+1 counters on each other creature you control equal to that creature's
/// toughness". Each object named gets its own number, read of that object: a loop over them, fixed as it begins, in which
/// the card the count reads (`Count$Card...`) is the object of the pass. One kind and a count of the card alone are read;
/// anything else stays a gap.
fn per_object_counters(ctx: &mut Ctx, p: &Params, name: &str) -> Tr<Vec<Value>> {
    if p.has("Choices")
        || p.has("CounterNum")
        || p.has("DividedAsYouChoose")
        || p.has("CounterTypes")
    {
        return unmapped("key:PutCounter.CounterNumPerDefined");
    }
    let Some(kind) = p.get("CounterType").and_then(kind_word) else {
        return unmapped("key:PutCounter.CounterType");
    };
    let script = ctx.svar(name).unwrap_or_default().to_owned();
    let card_only = script
        .split(['/', '.'])
        .filter(|part| part.starts_with("Count$"))
        .all(|part| {
            matches!(
                part,
                "Count$CardToughness"
                    | "Count$CardPower"
                    | "Count$CardBasePower"
                    | "Count$CardNumColors"
            )
        });
    if !script.starts_with("Count$Card") || !card_only {
        return unmapped("key:PutCounter.CounterNumPerDefined");
    }
    // `Defined$ Targeted` beside the target says the target again.
    let over = if p.get("Defined") == Some("Targeted") && p.has("ValidTgts") {
        subject(ctx, &p.without(&["Defined"]), "Self", None)?
    } else {
        subject(ctx, p, "Self", None)?
    };
    let count = amount(ctx, name)?;
    let each = ctx.fresh("each");
    let count = g9d_of_each(count, &json!({ "Bound": each }));
    Ok(vec![
        json!({"RepeatEach": {"over": over, "as": each, "effects": [
        {"PutCounters": {"on": {"Bound": each}, "counter": kind, "count": count}}]}}),
    ])
}

/// The count with every object it reads of the source (`This`) read of `each` instead.
fn g9d_of_each(count: Value, each: &Value) -> Value {
    match count {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    if key == "of" && value == json!("This") {
                        (key, each.clone())
                    } else {
                        (key, g9d_of_each(value, each))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| g9d_of_each(item, each))
                .collect(),
        ),
        other => other,
    }
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
    if matches!(p.get("CounterType"), Some("ALL" | "All"))
        && matches!(p.get("CounterNum"), None | Some("All"))
    {
        return Ok(json!({"RemoveAllCounters": {"from": from}}));
    }
    // Counters of any kinds, picked one at a time by the controller (CR 122.1).
    if p.get("CounterType") == Some("Any") {
        if matches!(p.get("CounterNum"), Some("All" | "Any")) {
            return unmapped("key:RemoveCounter.Any-number");
        }
        let count = amount(ctx, p.get("CounterNum").unwrap_or("1"))?;
        return Ok(json!({"RemoveCounters": {"from": from, "count": count}}));
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
    // "Sacrifice any number of": an optional sacrifice of more than one takes as many as the player chooses.
    let up_to = may && p.get("Amount").is_some_and(|n| n != "1");
    let who = frame::player_actor(ctx, p, &SACRIFICE)?;
    let filter = match p.get("SacValid") {
        None | Some("Self") if who == json!("You") => vec![json!("This")],
        None | Some("Self") => return unmapped("key:Sacrifice.SacValid"),
        Some(valid) => selector_any(ctx, valid)?,
    };
    let count = frame::count(ctx, p, &SACRIFICE)?.max;
    let verb = if up_to { "SacrificeUpTo" } else { "Sacrifice" };
    let mut effect = json!({verb: {"who": who.clone(), "filter": filter, "count": count}});
    if remember::wanted(p, &["RememberSacrificed"]) {
        remember::bind_moved(ctx, &mut effect, verb, "sacrificed");
    }
    if may && !up_to {
        // The player who sacrifices decides, not the controller of the ability.
        effect = super::frame::ask(vec![effect], &who).remove(0);
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
        // "For as long as ~ remains tapped" ends when the source untaps or leaves the battlefield.
        Some("Untap,LeavesPlay") => "WhileSourceTapped",
        // "For as long as you control ~" also ends when another player takes the source (CR 611.2b).
        Some("LeavesPlay,LoseControl") => "WhileYouControlSource",
        Some("LeavesPlay") => "WhileSourceOnBattlefield",
        Some(other) => return unmapped(format!("key:GainControl.LoseControl={other}")),
    };
    // A target player with a `Defined$` object: the target takes control of that object ("target opponent gains control
    // of ~"); a `Defined$` beside a target object names that target again.
    let mut target_player = None;
    let affected = match (p.get("AllValid"), p.get("Defined")) {
        (Some(valid), _) => json!({"Each": group_of_player(ctx, p, valid)?}),
        (None, Some(word)) if p.has("ValidTgts") => {
            let spec = super::valid::target_spec(ctx, p, None)?;
            let player = matches!(spec["what"].as_str(), Some("Player" | "Opponent"));
            let target = ctx.target(spec)?;
            if player {
                target_player = Some(target);
            } else if !word.starts_with("Targeted") && !word.starts_with("ThisTargeted") {
                return unmapped(format!("key:GainControl.Defined={word}-beside-a-target"));
            }
            objects(defined(ctx, word)?)?
        }
        (None, _) => objects(subject(ctx, p, "", None)?)?,
    };
    let controller = match p.get("NewController") {
        None => target_player.unwrap_or_else(|| json!("You")),
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
            "Choices",
            "ChoiceTitle",
        ],
    )?;
    let object = p.get("Object");
    if object.is_some_and(|word| word.starts_with("Valid ")) {
        return unmapped("key:Attach.Object-all");
    }
    let mut effects = Vec::new();
    let (what, to) = if let Some(choices) = p.get("Choices") {
        // The player picks, as the effect resolves: the attachment (for a host named by `Defined$`) or the host (for an
        // `Object$`).
        if p.has("ValidTgts") || (object.is_some() == p.has("Defined")) {
            return unmapped("key:Attach.Choices-shape");
        }
        let chosen = ctx.fresh("attach");
        let from = json!({"Each": selector_in(ctx, choices, None)?});
        effects.push(json!({"Choose": {
            "who": "You", "from": from, "count": {"min": 1, "max": 1}, "as": chosen}}));
        let bound = json!({ "Bound": chosen });
        match (object, p.get("Defined")) {
            (Some(word), None) => (defined(ctx, word)?, bound),
            (None, Some(host)) => (bound, defined(ctx, host)?),
            _ => return unmapped("key:Attach.Choices-shape"),
        }
    } else {
        attach_ends(ctx, p, object)?
    };
    effects.push(json!({"Attach": {"what": what, "to": to}}));
    if p.get("Optional") == Some("True") {
        effects = super::frame::ask(effects, &json!("You"));
    }
    Ok(effects)
}

/// The attachment and the host of an attach line with no `Choices$`.
fn attach_ends(ctx: &mut Ctx, p: &Params, object: Option<&str>) -> Tr<(Value, Value)> {
    Ok(match (object, p.has("ValidTgts"), p.get("Defined")) {
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
    })
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
    if word == super::damaged_way::DAMAGED_THIS_WAY {
        return super::damaged_way::shield(ctx).map(Some);
    }
    let kind = match word {
        "Targeted" | "ThisTargetedCard" | "ThisTargetedCard.Creature" if p.has("ValidTgts") => {
            ctx.last_target()?
        }
        _ => return unmapped(format!("key:ReplaceDyingDefined={word}")),
    };
    // A planeswalker target dies as well (CR 700.4): "if that creature or planeswalker would die this turn".
    let dying = match p.get("ValidTgts") {
        Some("Creature,Planeswalker") => {
            json!({"AnyOf": [{"Type": "Creature"}, {"Type": "Planeswalker"}]})
        }
        _ => json!({"Type": "Creature"}),
    };
    let shield = json!({"Shield": {
        "when": {"ZoneChange": {
            "what": [{"Is": kind}, dying], "from": "Battlefield", "to": "Graveyard"}},
        "by": {"MoveTo": "Exile"},
        "duration": "UntilEndOfTurn"}});
    Ok(Some(shield))
}
