//! Static abilities: an `S:` line as the DSL's `Static`, `Rule` or `CostModifier` ability.

use serde_json::{Value, json};

use super::amount::amount;
use super::condition::{condition, strip};
use super::cost::cost;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::keyword::keyword_list;
use super::valid::{defined, selector_any, selector_in};
use crate::forge::script::Params;

/// The spells cast this turn that a condition of a line about spells on the stack counts are those cast before the spell it
/// affects: "the first spell you cast each turn" is about the spell itself, which a cast trigger already finds logged.
fn before_itself(mut condition: Value) -> Value {
    match &mut condition {
        Value::Object(fields) => {
            for (key, inner) in fields.iter_mut() {
                if key == "Tally"
                    && inner.get("event") == Some(&json!("SpellCast"))
                    && let Some(filter) = inner.get_mut("filter").and_then(Value::as_array_mut)
                {
                    filter.push(json!({"Not": {"Is": "Affected"}}));
                } else {
                    *inner = before_itself(inner.take());
                }
            }
        }
        Value::Array(items) => {
            for inner in items.iter_mut() {
                *inner = before_itself(inner.take());
            }
        }
        _ => {}
    }
    condition
}

/// A static line as an ability.
pub fn static_line(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    ctx.reset();
    let all = Params::parse(body);
    let condition = condition(ctx, &all)?;
    let condition = match condition {
        Some(found) if all.get("AffectedZone") == Some("Stack") => Some(before_itself(found)),
        other => other,
    };
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
    if let Some(found) = super::statics_combat::ability(ctx, p) {
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
        (Some(word), Some(_))
            if matches!(word, "Self" | "Enchanted" | "Equipped" | "AttachedBy Self") =>
        {
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
        // What this permanent is attached to: the enchanted or equipped object.
        "AttachedBy Self" => Ok(json!("Attached")),
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
const CONTINUOUS: [&str; 27] = [
    "CanBlockAmount",
    "CanBlockAny",
    "GainsAbilitiesOf",
    "GainsAbilitiesOfZones",
    "AddStaticAbility",
    "RemoveCreatureTypes",
    "AddAllCreatureTypes",
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

/// The amount of a power or toughness change; one named `AffectedX` is worked out for each object the line affects.
fn stat(ctx: &Ctx, text: &str) -> Tr<Value> {
    let value = amount(ctx, text)?;
    if text.trim_start_matches('-').starts_with("AffectedX") {
        return Ok(per_affected(value));
    }
    Ok(value)
}

/// An amount read from the object each change applies to: `This` and `Other` of its selectors name that object.
fn per_affected(value: Value) -> Value {
    let is_affected = || json!({"Is": "Affected"});
    match value {
        Value::String(word) if word == "This" => json!("Affected"),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| match item.as_str() {
                    Some("This") => is_affected(),
                    Some("Other") => json!({"Not": is_affected()}),
                    _ => per_affected(item),
                })
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, item)| (key, per_affected(item)))
                .collect(),
        ),
        other => other,
    }
}

#[allow(clippy::too_many_lines)]
fn continuous(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Continuous", &CONTINUOUS)?;
    let affected = affected(ctx, p)?;
    let refinement = refinement(ctx, p, &affected)?;
    let mut mods = Vec::new();
    if p.has("AddPower") || p.has("AddToughness") {
        let power = stat(ctx, p.get("AddPower").unwrap_or("0"))?;
        let toughness = stat(ctx, p.get("AddToughness").unwrap_or("0"))?;
        mods.push(json!({"ModifyPT": {"power": power, "toughness": toughness}}));
    }
    match (p.get("SetPower"), p.get("SetToughness")) {
        (Some(power), Some(toughness)) => {
            let (power, toughness) = (stat(ctx, power)?, stat(ctx, toughness)?);
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
        (None, None) => {}
        (power, toughness) => {
            let (power, toughness) = half_stats(ctx, p, power, toughness)?;
            mods.push(json!({"SetPT": {"power": power, "toughness": toughness}}));
        }
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
    mods.extend(super::statics_combat::block_capacity(p)?);
    if p.get("RemoveAllAbilities") == Some("True") {
        mods.push(json!("LoseAbilities"));
    }
    match p.get("GainControl") {
        None => {}
        Some("You") => mods.push(json!({"SetController": "You"})),
        Some(other) => return unmapped(format!("key:Static.Continuous.GainControl={other}")),
    }
    if let Some(colors) = p.get("SetColor") {
        mods.push(json!({"SetColors": color_set(colors)?}));
    }
    let remove = p.get("RemoveCreatureTypes") == Some("True");
    match p.get("AddType") {
        Some(types) => mods.extend(added_types(ctx, types, remove)?),
        None if remove => mods.push(json!({"SetCreatureTypes": []})),
        None => {}
    }
    if p.get("AddAllCreatureTypes") == Some("True") {
        mods.push(json!("AddAllCreatureTypes"));
    }
    for (key, trigger) in [("AddAbility", false), ("AddTrigger", true)] {
        if let Some(names) = p.get(key) {
            mods.extend(granted(ctx, names, trigger)?);
        }
    }
    if let Some(valid) = p.get("GainsAbilitiesOf") {
        mods.push(gains_abilities(ctx, p, valid)?);
    }
    if let Some(names) = p.get("AddStaticAbility") {
        mods.extend(granted_statics(ctx, names)?);
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
fn added_types(ctx: &Ctx, text: &str, replacing: bool) -> Tr<Vec<Value>> {
    let (mut mods, mut subtypes) = (Vec::new(), Vec::new());
    for word in text.split('&').map(str::trim) {
        if super::ctx::CARD_TYPES.contains(&word) {
            mods.push(json!({"AddType": word}));
        } else if ctx.world.subtypes.contains(word) {
            subtypes.push(word);
        } else {
            return unmapped(format!("key:Static.Continuous.AddType={word}"));
        }
    }
    // "Is a Spirit" with `RemoveCreatureTypes$` replaces the creature types; without it the subtypes join them.
    if replacing {
        mods.push(json!({"SetCreatureTypes": subtypes}));
    } else {
        mods.extend(subtypes.iter().map(|name| json!({"AddSubtype": name})));
    }
    Ok(mods)
}

/// `SetColor$`: the colours the objects become; `Colorless` and `All` are the empty and the full set. A chosen colour is a gap.
fn color_set(text: &str) -> Tr<Vec<&str>> {
    const COLORS: [&str; 5] = ["White", "Blue", "Black", "Red", "Green"];
    match text {
        "Colorless" => Ok(Vec::new()),
        "All" => Ok(COLORS.to_vec()),
        list => list
            .split(',')
            .map(|word| {
                COLORS
                    .iter()
                    .copied()
                    .find(|color| *color == word)
                    .map_or_else(
                        || unmapped(format!("key:Static.Continuous.SetColor={word}")),
                        Ok,
                    )
            })
            .collect(),
    }
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
            if !only.iter().any(|pred| pred.get("Not").is_some()) {
                // "Can't block X" is "blocks only what is not X"; not (a and b) is (not a) or (not b).
                let negated: Vec<Value> = only.iter().map(|pred| json!({"Not": pred})).collect();
                let outside = match negated.as_slice() {
                    [single] => single.clone(),
                    _ => json!({"AnyOf": negated}),
                };
                return Ok(
                    json!({"Static": {"affected": "This", "mods": [{"CanBlockOnly": [outside]}]}}),
                );
            }
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

/// The kind of activated ability a `ReduceCost` or `RaiseCost` line changes the cost of, `None` for the cost of a spell.
fn activation_kind(p: &Params) -> Tr<Option<&'static str>> {
    let spell = p.get("ValidSpell");
    if p.get("Type") == Some("Ability") {
        return match spell {
            None | Some("Activated.!ManaAbility") => Ok(Some("Any")),
            Some(other) => unmapped(format!("key:Static.CostModifier.ValidSpell={other}")),
        };
    }
    match spell {
        None => Ok(None),
        Some("Activated.Equip") => Ok(Some("Equip")),
        Some("Activated.Cycling") => Ok(Some("Cycling")),
        Some("Activated.!ManaAbility") => Ok(Some("Any")),
        Some(other) => unmapped(format!("key:Static.CostModifier.ValidSpell={other}")),
    }
}

/// `ReduceCost` and `RaiseCost` of spells and of activated abilities.
fn cost_modifier(ctx: &Ctx, p: &Params, change: &str) -> Tr<Value> {
    keys(
        p,
        "CostModifier",
        &[
            "ValidCard",
            "ValidTarget",
            "ValidSpell",
            "Color",
            "Type",
            "Activator",
            "Amount",
            "EffectZone",
            "AffectedZone",
            "MinMana",
            "Relative",
            "OnlyFirstSpell",
            "Condition",
        ],
    )?;
    let activating = activation_kind(p)?;
    if activating.is_none() && p.get("Type") != Some("Spell") {
        return unmapped("static:CostModifier-Type");
    }
    if !matches!(p.get("AffectedZone"), None | Some("Battlefield")) {
        return unmapped("key:Static.CostModifier.AffectedZone");
    }
    // A spell's own cost applies in hand and on the stack; any other line works on the battlefield.
    match (p.get("EffectZone"), p.get("ValidCard")) {
        (None | Some("Battlefield"), _) | (Some(_), Some("Card.Self")) => {}
        _ => return unmapped("key:Static.CostModifier.EffectZone"),
    }
    if activating.is_some() && p.has("ValidTarget") {
        return unmapped("key:Static.CostModifier.ValidTarget-of-an-ability");
    }
    let mut applies = match p.get("ValidCard").unwrap_or("Card") {
        "Card.Self" => {
            if p.has("Activator") && activating.is_none() {
                return unmapped("static:CostModifier-self-with-Activator");
            }
            vec![json!("This")]
        }
        valid => {
            let mut selector = if activating.is_some() {
                Vec::new()
            } else {
                vec![json!("Spell")]
            };
            if valid != "Card" || activating.is_none() {
                selector.extend(selector_any(ctx, valid)?);
            }
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
    if let Some(target) = p.get("ValidTarget") {
        // "Spells that target a tapped creature": the cost is judged with the targets the spell has announced.
        applies.push(json!({"Targeting": selector_any(ctx, target)?}));
    }
    let mut conditions = Vec::new();
    if let Some(name) = p.get("Condition") {
        conditions.push(super::condition::named(name)?);
    }
    if p.get("OnlyFirstSpell") == Some("True") {
        // "The first spell of that kind you cast each turn": none of that kind was cast before.
        if activating.is_some() || p.get("ValidCard").is_none() || p.get("Activator") != Some("You")
        {
            return unmapped("key:Static.CostModifier.OnlyFirstSpell-shape");
        }
        let filter = selector_any(ctx, p.get("ValidCard").unwrap_or_default())?;
        conditions.push(json!({"Compare": {
            "left": {"Tally": {"event": "SpellCast", "who": "You", "filter": filter}},
            "op": "Eq", "right": 0}}));
    } else if p.has("OnlyFirstSpell") {
        return unmapped("key:Static.CostModifier.OnlyFirstSpell-value");
    }
    let mut built = modifier(ctx, p, change, &applies)?;
    if let Some(kind) = activating {
        built["CostModifier"]["activating"] = json!(kind);
    }
    match conditions.len() {
        0 => {}
        1 => built["CostModifier"]["condition"] = conditions.remove(0),
        _ => built["CostModifier"]["condition"] = json!({"All": conditions}),
    }
    if let Some(least) = p.get("MinMana") {
        let Ok(least) = least.parse::<u32>() else {
            return unmapped("key:Static.CostModifier.MinMana-not-a-number");
        };
        built["CostModifier"]["least"] = json!(least);
    }
    Ok(built)
}

/// The `CostModifier` ability for the amount of mana a line changes by.
fn modifier(ctx: &Ctx, p: &Params, change: &str, applies: &[Value]) -> Tr<Value> {
    let amount = p.get("Amount").unwrap_or_default();
    // `Color$` names the one colour whose symbols the change adds or removes, `Amount$` how many.
    // "{U} (or {1}) less" takes a coloured symbol when the cost has one and generic mana when it has not, which a
    // reduction of one symbol does not say.
    if p.has("Color") && ctx.oracle.contains("(or {1})") {
        return unmapped("key:Static.CostModifier.Color-or-generic");
    }
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
    if p.get("Relative").is_some() {
        return unmapped("key:Static.CostModifier.Relative-with-a-number");
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

/// A characteristic-defining line that sets only one of power and toughness (`*/4` and `SetPower$ X`): the other half is the
/// number the card prints. Any other line with one half would lose the half it leaves, so it stays a gap.
fn half_stats(
    ctx: &Ctx,
    p: &Params,
    power: Option<&str>,
    toughness: Option<&str>,
) -> Tr<(Value, Value)> {
    let gap = || unmapped("key:Static.Continuous.half-SetPT");
    let (Some(printed), Some("True")) = (ctx.pt, p.get("CharacteristicDefining")) else {
        return gap();
    };
    let Some((left, right)) = printed.split_once('/') else {
        return gap();
    };
    let fixed = |text: &str| text.parse::<u32>().ok().map(|n| json!(n));
    match (power, toughness) {
        (Some(set), None) if left == "*" => {
            let Some(other) = fixed(right) else {
                return gap();
            };
            Ok((amount(ctx, set)?, other))
        }
        (None, Some(set)) if right == "*" => {
            let Some(other) = fixed(left) else {
                return gap();
            };
            Ok((other, amount(ctx, set)?))
        }
        _ => gap(),
    }
}

/// `AddStaticAbility$`: the named `S:` parts as static abilities the affected objects gain. Only a static (not a rule or a
/// cost modifier) and one that gives nothing in turn can be granted.
fn granted_statics(ctx: &mut Ctx, names: &str) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    for name in names.split('&').map(str::trim) {
        let Some(text) = ctx.svar(name) else {
            return unmapped("key:Static.Continuous.AddStaticAbility-not-a-part");
        };
        // A granted static grants nothing in turn; reading it first also stops a part that names itself.
        if text.contains("AddStaticAbility")
            || text.contains("AddAbility")
            || text.contains("AddTrigger")
        {
            return unmapped("key:Static.Continuous.AddStaticAbility-shape");
        }
        let ability = static_line(ctx, text)?;
        let Some(inner) = ability.get("Static") else {
            return unmapped("key:Static.Continuous.AddStaticAbility-not-a-static");
        };
        if inner.get("defining").is_some()
            || inner.to_string().contains("\"GrantAbility\"")
            || inner.get("from").is_some()
        {
            return unmapped("key:Static.Continuous.AddStaticAbility-shape");
        }
        mods.push(json!({"GrantAbility": ability}));
    }
    Ok(mods)
}

/// `GainsAbilitiesOf$`: the object has the activated abilities printed on the cards of a valid-string in a graveyard,
/// in exile or on top of a library. A card on the battlefield may have other abilities than it prints, which the node
/// does not read, so that stays a gap.
fn gains_abilities(ctx: &Ctx, p: &Params, valid: &str) -> Tr<Value> {
    if p.has("GainsValidAbilities") {
        return unmapped("key:Static.Continuous.GainsValidAbilities");
    }
    let zone = match p.get("GainsAbilitiesOfZones") {
        Some(zone @ ("Graveyard" | "Exile" | "Library")) => zone,
        Some(other) => {
            return unmapped(format!(
                "key:Static.Continuous.GainsAbilitiesOfZones={other}"
            ));
        }
        None => return unmapped("key:Static.Continuous.GainsAbilitiesOf-battlefield"),
    };
    Ok(json!({"GainAbilitiesOf": {"Each": selector_in(ctx, valid, Some(zone))?}}))
}
