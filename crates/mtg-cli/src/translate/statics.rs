//! Static abilities: an `S:` line as the DSL's `Static`, `Rule` or `CostModifier` ability.

use serde_json::{Value, json};

use super::amount::amount;
use super::condition::{condition, strip};
use super::cost::cost;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::keyword::keyword_list;
use super::valid::{defined, selector_any, selector_in};
use crate::forge::script::Params;

/// A static line as an ability.
pub fn static_line(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    ctx.reset();
    let all = Params::parse(body);
    let condition = condition(ctx, &all)?;
    let p = strip(&all);
    let mut ability = match super::statics_rules::rule(ctx, &p) {
        Some(found) => found?,
        None => by_mode(ctx, &p)?,
    };
    if condition.is_some() && ability.pointer("/Static/defining").is_some() {
        return unmapped("key:Static.Continuous.defining-with-condition");
    }
    if let Some(condition) = condition
        && let Some(inner) = ability.as_object_mut().and_then(|o| o.values_mut().next())
    {
        inner["condition"] = match inner.get("condition") {
            Some(earlier) => json!({"All": [earlier, condition]}),
            None => condition,
        };
    }
    Ok(ability)
}

/// The ability of a static line by its mode.
fn by_mode(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    if let Some(found) = super::effects_animate::animate_words(ctx, p) {
        return found;
    }
    Ok(match p.get("Mode").unwrap_or_default() {
        "Continuous" if p.has("MayPlay") => {
            json!({"Rule": {"rule": super::mayplay::may_cast(ctx, p, None)?}})
        }
        "Continuous" => continuous(ctx, p)?,
        "CantAttack" => restriction(ctx, p, &["CantAttack"])?,
        "CantBlock" => restriction(ctx, p, &["CantBlock"])?,
        "CantAttack,CantBlock" | "CantBlock,CantAttack" => {
            restriction(ctx, p, &["CantAttack", "CantBlock"])?
        }
        "CantBlockBy" => cant_block_by(ctx, p)?,
        "MustAttack" => must_act(ctx, p, "MustAttack")?,
        "MustBlock" => must_act(ctx, p, "MustBlock")?,
        "ReduceCost" => cost_modifier(ctx, p, "Reduce")?,
        "RaiseCost" => cost_modifier(ctx, p, "Increase")?,
        "AlternativeCost" => super::statics_alt::alternative_cost(ctx, p)?,
        other => return unmapped(format!("static:{other}")),
    })
}

/// Refuses a key the mode does not read.
fn keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    check_keys(p, &format!("Static.{mode}"), read, |key, _| key == "Mode")
}

/// The objects a static line affects: `Affected$` or `AffectedDefined$`.
fn affected(ctx: &Ctx, p: &Params) -> Tr<Value> {
    if p.has("AffectedZone") && (p.has("AffectedDefined") || !p.has("Affected")) {
        return unmapped("key:Static.Continuous.AffectedZone-without-Affected");
    }
    match (p.get("AffectedDefined"), p.get("Affected")) {
        (Some(word), None | Some("Creature" | "Card" | "Permanent")) => static_defined(ctx, word),
        (Some(word), Some(_)) if matches!(word, "Self" | "Enchanted" | "Equipped") => {
            static_defined(ctx, word)
        }
        (Some(_), Some(_)) => unmapped("static-affected:Defined+Affected"),
        (None, Some("You")) if !p.has("AffectedZone") => Ok(json!("You")),
        (None, Some("Player")) if !p.has("AffectedZone") => Ok(json!("EachPlayer")),
        (None, Some("Opponent")) if !p.has("AffectedZone") => Ok(json!("Opponents")),
        (None, Some(valid)) => zoned(ctx, valid, p.get("AffectedZone")),
        (None, None) if p.get("CharacteristicDefining") == Some("True") => Ok(json!("This")),
        (None, None) => unmapped("static-affected:none"),
    }
}

fn static_defined(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word {
        "Self" | "Enchanted" | "Equipped" => defined(ctx, word),
        other => unmapped(format!("static-affected:{other}")),
    }
}

/// The condition an `Affected$` refinement of a defined object states: "it has first strike while it attacks" affects the
/// object only while it matches the valid-string. A plain type word adds none.
fn refinement(ctx: &Ctx, p: &Params, subject: &Value) -> Tr<Option<Value>> {
    let (Some(_), Some(valid)) = (p.get("AffectedDefined"), p.get("Affected")) else {
        return Ok(None);
    };
    if matches!(valid, "Creature" | "Card" | "Permanent") {
        return Ok(None);
    }
    if p.has("AffectedZone") {
        return unmapped("key:Static.Continuous.AffectedZone-with-AffectedDefined");
    }
    let selector = selector_any(ctx, valid)?;
    Ok(Some(
        json!({"Matches": {"subject": subject, "selector": selector}}),
    ))
}

/// The objects of a valid-string in the zone `AffectedZone$` names: a single zone other than the battlefield, whose cards
/// the layers hold like permanents.
fn zoned(ctx: &Ctx, valid: &str, zone: Option<&str>) -> Tr<Value> {
    match zone {
        None | Some("Battlefield") => affected_valid(ctx, valid),
        Some(zone @ ("Hand" | "Graveyard" | "Exile" | "Library")) => {
            Ok(json!({"Each": selector_in(ctx, valid, Some(zone))?}))
        }
        Some("Stack") => {
            let mut preds = selector_any(ctx, valid)?;
            preds.push(json!({"In": "Stack"}));
            Ok(json!({"Each": preds}))
        }
        Some(other) => unmapped(format!("key:Static.Continuous.AffectedZone={other}")),
    }
}

fn affected_valid(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word {
        "Card.Self" | "Creature.Self" => Ok(json!("This")),
        "Card.EnchantedBy" | "Creature.EnchantedBy" | "Card.EquippedBy" | "Creature.EquippedBy" => {
            Ok(json!("Attached"))
        }
        "You" | "Player" | "Opponent" => unmapped(format!("static-affected:{word}")),
        valid => Ok(json!({"Each": selector_any(ctx, valid)?})),
    }
}

/// The keys a continuous static line reads.
const CONTINUOUS: [&str; 20] = [
    "AddHiddenKeyword",
    "AddSVar",
    "AffectedZone",
    "AddAbility",
    "AddTrigger",
    "AddType",
    "Affected",
    "AffectedDefined",
    "AddPower",
    "AddToughness",
    "SetPower",
    "SetToughness",
    "AddKeyword",
    "RemoveKeyword",
    "RemoveAllAbilities",
    "EffectZone",
    "CharacteristicDefining",
    "GainControl",
    "AddType",
    "SetColor",
];

fn continuous(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Continuous", &CONTINUOUS)?;
    let affected = affected(ctx, p)?;
    let refinement = refinement(ctx, p, &affected)?;
    let mut mods = Vec::new();
    if p.has("AddPower") || p.has("AddToughness") {
        let power = amount(ctx, p.get("AddPower").unwrap_or("0"))?;
        let toughness = amount(ctx, p.get("AddToughness").unwrap_or("0"))?;
        mods.push(json!({"ModifyPT": {"power": power, "toughness": toughness}}));
    }
    match (p.get("SetPower"), p.get("SetToughness")) {
        (Some(power), Some(toughness)) => {
            let (power, toughness) = (amount(ctx, power)?, amount(ctx, toughness)?);
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
        (None, None) => {}
        _ => return unmapped("key:Static.Continuous.half-SetPT"),
    }
    for (key, variant) in [
        ("AddKeyword", "AddKeyword"),
        ("RemoveKeyword", "RemoveKeyword"),
    ] {
        if let Some(keywords) = p.get(key) {
            for keyword in keyword_list(ctx, keywords)? {
                mods.push(json!({ variant: keyword }));
            }
        }
    }
    if let Some(word) = p.get("AddHiddenKeyword") {
        // Only the combat restrictions have a modification; any other hidden keyword is another rule.
        match word.trim_end_matches('.') {
            "CARDNAME can't block" => mods.push(json!("CantBlock")),
            "CARDNAME can't attack" => mods.push(json!("CantAttack")),
            "CARDNAME can't attack or block" => {
                mods.extend([json!("CantAttack"), json!("CantBlock")]);
            }
            _ => return unmapped("key:Static.Continuous.AddHiddenKeyword"),
        }
    }
    if p.get("RemoveAllAbilities") == Some("True") {
        mods.push(json!("LoseAbilities"));
    }
    match p.get("GainControl") {
        None => {}
        Some("You") => mods.push(json!({"SetController": "You"})),
        Some(other) => return unmapped(format!("key:Static.Continuous.GainControl={other}")),
    }
    if p.has("SetColor") {
        return unmapped("key:Static.Continuous.SetColor");
    }
    if let Some(types) = p.get("AddType") {
        mods.extend(added_types(ctx, types)?);
    }
    for (key, trigger) in [("AddAbility", false), ("AddTrigger", true)] {
        if let Some(names) = p.get(key) {
            mods.extend(granted(ctx, names, trigger)?);
        }
    }
    if mods.is_empty() {
        return unmapped("static:Continuous-nothing");
    }
    // Only the keywords a player can have apply to players; any other change would be lost.
    if matches!(affected.as_str(), Some("You" | "EachPlayer" | "Opponents"))
        && !mods.iter().all(|m| {
            m.get("AddKeyword")
                .is_some_and(|k| k == "Hexproof" || k == "Shroud")
        })
    {
        return unmapped(format!(
            "static-affected:{}",
            p.get("Affected").unwrap_or_default()
        ));
    }
    let mut ability = json!({"affected": affected, "mods": mods});
    if let Some(refinement) = refinement {
        ability["condition"] = refinement;
    }
    match p.get("EffectZone") {
        None | Some("Battlefield") => {}
        Some(zone @ ("Graveyard" | "Hand" | "Exile" | "Library")) => ability["from"] = json!(zone),
        Some(other) => return unmapped(format!("key:Static.EffectZone={other}")),
    }
    if p.get("CharacteristicDefining") == Some("True") {
        // Only power and toughness are defined this way here, and always, never under a condition.
        if mods
            .iter()
            .any(|modification| modification.get("SetPT").is_none())
        {
            return unmapped("key:Static.Continuous.defining-other-than-stats");
        }
        ability["defining"] = json!(true);
    }
    Ok(json!({ "Static": ability }))
}

/// `AddType$ Artifact & Zombie`: the card types and subtypes a static line adds.
fn added_types(ctx: &Ctx, text: &str) -> Tr<Vec<Value>> {
    text.split('&')
        .map(str::trim)
        .map(|word| {
            if super::ctx::CARD_TYPES.contains(&word) {
                Ok(json!({"AddType": word}))
            } else if ctx.world.subtypes.contains(word) {
                Ok(json!({"AddSubtype": word}))
            } else {
                unmapped(format!("key:Static.Continuous.AddType={word}"))
            }
        })
        .collect()
}

/// `AddAbility$` and `AddTrigger$`: the named parts, translated, as abilities the affected objects gain.
fn granted(ctx: &mut Ctx, names: &str, trigger: bool) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    for name in names.split('&').map(str::trim) {
        let Some(text) = ctx.svar(name) else {
            return unmapped("key:Static.Continuous.AddAbility-not-a-part");
        };
        let abilities = if trigger {
            vec![super::triggers::triggered(ctx, text)?]
        } else {
            super::ability::ability_line(ctx, text)?
        };
        mods.extend(abilities.into_iter().map(|a| json!({"GrantAbility": a})));
    }
    Ok(mods)
}

/// `CantAttack`, `CantBlock` of the cards named by `ValidCard$`.
fn restriction(ctx: &Ctx, p: &Params, modifications: &[&str]) -> Tr<Value> {
    keys(p, modifications[0], &["ValidCard", "Target"])?;
    let affected = affected_valid(ctx, p.get("ValidCard").unwrap_or_default())?;
    if let Some(target) = p.get("Target") {
        // "Can't attack you" keeps the players and planeswalkers it protects apart.
        let attacked = match target {
            "You" => "Player",
            "Planeswalker.YouCtrl" => "Planeswalkers",
            "You,Planeswalker.YouCtrl" => "Both",
            _ => return unmapped(format!("key:Static.CantAttack.Target={target}")),
        };
        if modifications != ["CantAttack"] {
            return unmapped("key:Static.CantBlock.Target");
        }
        return Ok(
            json!({"Static": {"affected": affected, "mods": [{"CantAttackYou": attacked}]}}),
        );
    }
    Ok(json!({"Static": {"affected": affected, "mods": modifications}}))
}

/// `CantBlockBy`: the attacker named by `ValidAttacker$` is not blocked by the blockers of `ValidBlocker$`, or a blocker
/// that is the card itself blocks only attackers that are not excluded.
fn cant_block_by(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CantBlockBy", &["ValidAttacker", "ValidBlocker"])?;
    let attacker = p.get("ValidAttacker").unwrap_or_default();
    let blocker = p.get("ValidBlocker");
    match (attacker, blocker) {
        ("Creature.Self" | "Card.Self", None) => {
            Ok(json!({"Static": {"affected": "This", "mods": ["CantBeBlocked"]}}))
        }
        ("Creature.Self" | "Card.Self", Some(valid)) => Ok(json!({"Static": {
            "affected": "This",
            "mods": [{"CantBeBlockedBy": selector_any(ctx, valid)?}],
        }})),
        (valid, Some("Creature.Self" | "Card.Self")) => {
            let mut only = selector_any(ctx, valid)?;
            let (Some(at), 1) = (
                only.iter().position(|pred| pred.get("Not").is_some()),
                only.iter().filter(|pred| pred.get("Not").is_some()).count(),
            ) else {
                return unmapped("static:CantBlockBy-shape");
            };
            // "Can't block X and not Y" is "blocks only Y" for an attacker that is a creature; any other filter would be lost.
            if only
                .iter()
                .enumerate()
                .any(|(i, pred)| i != at && *pred != json!({"Type": "Creature"}))
            {
                return unmapped("static:CantBlockBy-extra-filter");
            }
            let inner = only.remove(at)["Not"].clone();
            only.push(inner);
            Ok(json!({"Static": {"affected": "This", "mods": [{"CanBlockOnly": only}]}}))
        }
        (valid, None) => Ok(json!({"Static": {
            "affected": affected_valid(ctx, valid)?,
            "mods": ["CantBeBlocked"],
        }})),
        (valid, Some(blockers)) => Ok(json!({"Static": {
            "affected": affected_valid(ctx, valid)?,
            "mods": [{"CantBeBlockedBy": selector_any(ctx, blockers)?}],
        }})),
    }
}

/// `MustAttack` and `MustBlock`: the creatures `ValidCreature$` names attack or block each combat if able.
fn must_act(ctx: &Ctx, p: &Params, rule: &str) -> Tr<Value> {
    keys(p, rule, &["ValidCreature"])?;
    let who = match p.get("ValidCreature").unwrap_or_default() {
        "Card.Self" | "Creature.Self" => json!("This"),
        "Creature.EnchantedBy" => json!("Attached"),
        valid => json!({"Each": selector_any(ctx, valid)?}),
    };
    Ok(json!({"Rule": {"rule": {rule: who}}}))
}

/// `ReduceCost` and `RaiseCost` of spells.
fn cost_modifier(ctx: &Ctx, p: &Params, change: &str) -> Tr<Value> {
    keys(
        p,
        "CostModifier",
        &[
            "ValidCard",
            "ValidTarget",
            "Color",
            "Type",
            "Activator",
            "Amount",
            "EffectZone",
        ],
    )?;
    if p.get("Type") != Some("Spell") {
        return unmapped("static:CostModifier-Type");
    }
    // A spell's own cost applies in hand and on the stack; any other line works on the battlefield.
    match (p.get("EffectZone"), p.get("ValidCard")) {
        (None | Some("Battlefield"), _) | (Some(_), Some("Card.Self")) => {}
        _ => return unmapped("key:Static.CostModifier.EffectZone"),
    }
    let applies = match p.get("ValidCard").unwrap_or("Card") {
        "Card.Self" => {
            if p.has("Activator") {
                return unmapped("static:CostModifier-self-with-Activator");
            }
            vec![json!("This")]
        }
        valid => {
            let mut selector = vec![json!("Spell")];
            selector.extend(selector_any(ctx, valid)?);
            match p.get("Activator") {
                None => {}
                Some("You") => selector.push(json!({"ControlledBy": "You"})),
                Some("Player.Opponent" | "Opponent") => {
                    selector.push(json!({"ControlledBy": "Opponents"}));
                }
                Some(other) => return unmapped(format!("static:CostModifier-Activator={other}")),
            }
            selector
        }
    };
    let mut applies = applies;
    if let Some(target) = p.get("ValidTarget") {
        // "Spells that target a tapped creature": the cost is judged with the targets the spell has announced.
        applies.push(json!({"Targeting": selector_any(ctx, target)?}));
    }
    let amount = p.get("Amount").unwrap_or_default();
    // `Color$` names the one colour whose symbols the change adds or removes, `Amount$` how many.
    let symbol = match p.get("Color") {
        None => "{1}".to_owned(),
        Some(color @ ("W" | "U" | "B" | "R" | "G")) => format!("{{{color}}}"),
        Some(other) => return unmapped(format!("key:Static.CostModifier.Color={other}")),
    };
    // A number or coloured symbols are the mana; a count (an `SVar`) is how many times {1} applies.
    if ctx.svar(amount).is_some() {
        let per = super::amount::amount(ctx, amount)?;
        return Ok(
            json!({"CostModifier": {"applies_to": applies, "change": change, "mana": symbol, "per": per}}),
        );
    }
    if p.has("Color") {
        let Ok(count) = amount.parse::<usize>() else {
            return unmapped("key:Static.CostModifier.Color-with-Amount");
        };
        return Ok(
            json!({"CostModifier": {"applies_to": applies, "change": change, "mana": symbol.repeat(count)}}),
        );
    }
    let Value::String(mana) = cost(ctx, amount)? else {
        return unmapped("static:CostModifier-Amount");
    };
    if mana.contains('X') {
        return unmapped("static:CostModifier-X");
    }
    Ok(json!({"CostModifier": {"applies_to": applies, "change": change, "mana": mana}}))
}
