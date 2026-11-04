//! Forge's object and player descriptions in our DSL: valid-strings (`Creature.YouCtrl+Other`) as selectors, `Defined$`
//! words as references, and `ValidTgts$` as target specifications.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{CARD_TYPES, Ctx, SUPERTYPES, Tr, unmapped};
use super::keyword::keyword_name;
use crate::forge::script::Params;

const COLORS: [&str; 5] = ["White", "Blue", "Black", "Red", "Green"];

/// A comparison operator word (`GE`) as the DSL names it.
pub fn cmp(word: &str) -> Option<&'static str> {
    Some(match word {
        "GE" => "Ge",
        "LE" => "Le",
        "EQ" => "Eq",
        "GT" => "Gt",
        "LT" => "Lt",
        "NE" => "Ne",
        _ => return None,
    })
}

/// The predicates a property of a valid-string stands for.
fn property(ctx: &Ctx, prop: &str) -> Tr<Vec<Value>> {
    let one = |value: Value| Ok(vec![value]);
    match prop {
        "YouCtrl" => return one(json!({"ControlledBy": "You"})),
        "OppCtrl" => return one(json!({"ControlledBy": "Opponents"})),
        "YouDontCtrl" => return one(json!({"Not": {"ControlledBy": "You"}})),
        "YouOwn" => return one(json!({"OwnedBy": "You"})),
        "OppOwn" => return one(json!({"OwnedBy": "Opponents"})),
        "Other" | "StrictlyOther" => return one(json!("Other")),
        "Self" | "StrictlySelf" => return one(json!("This")),
        "tapped" | "Tapped" => return one(json!("Tapped")),
        "untapped" | "Untapped" => return one(json!("Untapped")),
        "attacking" => return one(json!("Attacking")),
        "blocking" => return one(json!("Blocking")),
        "token" | "Token" => return one(json!("Token")),
        "nonToken" => return one(json!({"Not": "Token"})),
        "Colorless" => return one(json!("Colorless")),
        "MultiColor" => return one(json!({"Compare": {"stat": "Colors", "op": "Ge", "to": 2}})),
        "EnchantedBy" | "EquippedBy" | "AttachedBy" => return one(json!({"Is": "Attached"})),
        "IsCommander" => return one(json!("Commander")),
        "modified" => return one(json!("Modified")),
        "IsGoaded" | "Goaded" => return one(json!("Goaded")),
        "HasCounters" => return one(json!("HasAnyCounter")),
        "hasXCost" => return one(json!("XInCost")),
        "RememberedPlayerCtrl" => {
            return match &ctx.remembered {
                Some(who) => one(json!({"ControlledBy": who})),
                None => unmapped("valid:RememberedPlayerCtrl-without-a-player"),
            };
        }
        "ChosenCard" => {
            return match &ctx.chosen_card {
                Some(name) => one(json!({"Is": {"Bound": name}})),
                None => unmapped("valid:ChosenCard-without-a-choice"),
            };
        }
        "ChosenColor" | "ChosenType" => {
            return match &ctx.chosen {
                Some(name) => one(json!({"ChosenAs": name})),
                None => unmapped(format!("valid:{prop}-without-a-choice")),
            };
        }
        "IsRemembered" => {
            return match &ctx.pinned {
                Some(who) => one(json!({"Is": who})),
                None => unmapped("valid:IsRemembered"),
            };
        }
        "ExiledWithSource" => return one(json!({"ExiledWith": "This"})),
        "enchanted" => return one(json!({"HasAttached": [{"Subtype": "Aura"}]})),
        "equipped" => return one(json!({"HasAttached": [{"Subtype": "Equipment"}]})),
        "Historic" => {
            return one(json!({"AnyOf": [
                {"Supertype": "Legendary"}, {"Type": "Artifact"}, {"Subtype": "Saga"}]}));
        }
        "inZoneGraveyard" => return one(json!({"In": "Graveyard"})),
        "inZoneHand" => return one(json!({"In": "Hand"})),
        "inZoneExile" => return one(json!({"In": "Exile"})),
        _ => {}
    }
    if let Some(preds) = super::valid_atoms::atom(prop) {
        return Ok(preds);
    }
    if let Some(preds) = typed_property(prop) {
        return Ok(preds);
    }
    if let Some(preds) = numeric_property(prop) {
        return Ok(preds);
    }
    if let Some(word) = prop.strip_prefix("with")
        && let Some(keyword) = keyword_name(&spaced(word))
    {
        return one(json!({"HasKeyword": keyword}));
    }
    if let Some(word) = prop.strip_prefix("without")
        && let Some(keyword) = keyword_name(&spaced(word))
    {
        return one(json!({"Not": {"HasKeyword": keyword}}));
    }
    if let Some(kind) = prop
        .strip_prefix("counters_GE1_")
        .and_then(super::amount::counter_kind)
    {
        return one(json!({"HasCounter": kind}));
    }
    if let Some(rest) = prop.strip_prefix("non")
        && ctx.world.subtypes.contains(&capitalize(rest))
    {
        return one(json!({"Not": {"Subtype": capitalize(rest)}}));
    }
    if prop.chars().next().is_some_and(char::is_uppercase) && ctx.world.subtypes.contains(prop) {
        return one(json!({"Subtype": prop}));
    }
    unmapped(format!("valid:{prop}"))
}

/// `FirstStrike` as `First Strike`.
fn spaced(word: &str) -> String {
    let mut out = String::new();
    for (at, c) in word.chars().enumerate() {
        if at > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// A card type, supertype or colour, or its negation (`nonLand`).
fn typed_property(prop: &str) -> Option<Vec<Value>> {
    let (negated, word) = match prop.strip_prefix("non") {
        Some(rest) => (true, capitalize(rest)),
        None => (false, prop.to_owned()),
    };
    let pred = if CARD_TYPES.contains(&word.as_str()) {
        json!({"Type": word})
    } else if SUPERTYPES.contains(&word.as_str()) {
        json!({"Supertype": word})
    } else if COLORS.contains(&word.as_str()) {
        json!({"Color": word})
    } else {
        return None;
    };
    Some(vec![if negated { json!({"Not": pred}) } else { pred }])
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `powerGE4`, `toughnessLE2`, `cmcEQ3`.
fn numeric_property(prop: &str) -> Option<Vec<Value>> {
    let (stat, rest) = [
        ("power", "Power"),
        ("toughness", "Toughness"),
        ("cmc", "ManaValue"),
    ]
    .iter()
    .find_map(|(word, stat)| prop.strip_prefix(word).map(|rest| (*stat, rest)))?;
    let op = cmp(rest.get(..2)?)?;
    let bound = rest.get(2..)?;
    let to = if bound == "X" {
        json!("X")
    } else {
        json!(bound.parse::<i32>().ok()?)
    };
    Some(vec![json!({"Compare": {"stat": stat, "op": op, "to": to}})])
}

/// The predicates of the base word of an alternative (`Creature`, `Permanent`, `Elf`).
fn base(ctx: &Ctx, word: &str) -> Tr<Vec<Value>> {
    if CARD_TYPES.contains(&word) {
        return Ok(vec![json!({"Type": word})]);
    }
    match word {
        "Permanent" | "Card" | "Any" => Ok(Vec::new()),
        "Spell" => Ok(vec![json!("Spell")]),
        _ if ctx.world.subtypes.contains(word) => Ok(vec![json!({"Subtype": word})]),
        _ => unmapped(format!("valid-base:{word}")),
    }
}

/// The negation of the predicates of one property (`!token`): only one predicate can be negated.
fn negated(mut preds: Vec<Value>) -> Tr<Value> {
    match (preds.pop(), preds.is_empty()) {
        (Some(pred), true) => Ok(json!({ "Not": pred })),
        _ => unmapped("valid:negation-of-several"),
    }
}

/// One alternative (`Creature.YouCtrl+Other`) as a selector.
pub fn selector(ctx: &Ctx, spec: &str) -> Tr<Vec<Value>> {
    let spec = spec.trim();
    let (head, props) = spec.split_once('.').unwrap_or((spec, ""));
    let mut preds = base(ctx, head)?;
    for prop in props.split('+').filter(|prop| !prop.is_empty()) {
        match prop.strip_prefix('!') {
            Some(inner) => preds.push(negated(property(ctx, inner)?)?),
            None => preds.extend(property(ctx, prop)?),
        }
    }
    // "Pirates" and "Elves" name the creatures of the type, "Pirate creatures" says it: the type word is left out beside a
    // creature type unless Forge's rendering of the text has it.
    let says_creature = |name: &str| ctx.oracle.contains(&format!("{name} creature"));
    let creature_type = preds.iter().any(|pred| {
        pred.get("Subtype")
            .and_then(Value::as_str)
            .is_some_and(|name| !NOT_CREATURE_TYPES.contains(&name) && !says_creature(name))
    });
    if creature_type {
        preds.retain(|pred| *pred != json!({"Type": "Creature"}));
    }
    Ok(preds)
}

/// Subtypes that are not creature types.
const NOT_CREATURE_TYPES: [&str; 22] = [
    "Aura",
    "Equipment",
    "Vehicle",
    "Fortification",
    "Saga",
    "Class",
    "Curse",
    "Shrine",
    "Treasure",
    "Food",
    "Clue",
    "Blood",
    "Gate",
    "Cave",
    "Desert",
    "Forest",
    "Island",
    "Mountain",
    "Plains",
    "Swamp",
    "Lesson",
    "Trap",
];

/// A comma-separated list of alternatives as a selector: one alternative, or alternatives that share their predicates but
/// one each (`Instant.YouCtrl,Sorcery.YouCtrl`, `Creature.attacking,Creature.blocking`).
pub fn selector_any(ctx: &Ctx, spec: &str) -> Tr<Vec<Value>> {
    let alternatives: Vec<&str> = spec.split(',').collect();
    if let [only] = alternatives.as_slice() {
        return selector(ctx, only);
    }
    let lists: Vec<Vec<Value>> = alternatives
        .iter()
        .map(|alternative| selector(ctx, alternative))
        .collect::<Tr<_>>()?;
    let common: Vec<Value> = lists[0]
        .iter()
        .filter(|pred| lists.iter().all(|list| list.contains(pred)))
        .cloned()
        .collect();
    let mut different = Vec::new();
    for list in &lists {
        let rest: Vec<&Value> = list.iter().filter(|pred| !common.contains(pred)).collect();
        match rest.as_slice() {
            [one] => different.push((*one).clone()),
            [] => return unmapped("valid:or-with-open-alternative"),
            // A conjunction as one alternative, by De Morgan: not any of the negations.
            many => {
                let negated: Vec<Value> = many.iter().map(|pred| json!({"Not": pred})).collect();
                different.push(json!({"Not": {"AnyOf": negated}}));
            }
        }
    }
    // The choice stands where the first alternative has its own predicate.
    let any = json!({"AnyOf": different});
    let mut placed = false;
    Ok(lists[0]
        .iter()
        .filter_map(|pred| {
            if common.contains(pred) {
                Some(pred.clone())
            } else if placed {
                None
            } else {
                placed = true;
                Some(any.clone())
            }
        })
        .collect())
}

/// A selector over the cards of a zone: the alternatives with `In(zone)` added when the zone is not the battlefield.
pub fn selector_in(ctx: &Ctx, spec: &str, zone: Option<&str>) -> Tr<Vec<Value>> {
    let mut preds = selector_any(ctx, spec)?;
    match zone {
        None | Some("Battlefield") => {}
        Some(zone @ ("Graveyard" | "Hand" | "Library" | "Exile")) => {
            // A card outside the battlefield is its owner's.
            for pred in &mut preds {
                if let Some(who) = pred.get("ControlledBy").cloned() {
                    *pred = json!({ "OwnedBy": who });
                }
            }
            preds.push(json!({ "In": zone }));
        }
        Some(other) => return unmapped(format!("zone:{other}")),
    }
    Ok(preds)
}

/// The reference a `Defined$` word stands for.
pub fn defined(ctx: &Ctx, word: &str) -> Tr<Value> {
    let controller = |inner: Value| Ok(json!({ "ControllerOf": inner }));
    match word.trim() {
        "You" => Ok(json!("You")),
        "Opponent" | "Player.Opponent" => Ok(json!("Opponents")),
        "Player" => Ok(json!("EachPlayer")),
        "Self" | "Card.Self" => Ok(json!("This")),
        "Sacrificed" => Ok(json!("Paid")),
        "Targeted" | "TargetedPlayer" | "TargetedCard" => ctx.last_target(),
        "ParentTarget" => ctx.parent(),
        "TargetedController" => controller(ctx.last_target()?),
        "TargetedOwner" => Ok(json!({ "OwnerOf": ctx.last_target()? })),
        "Enchanted" | "Equipped" => Ok(json!("Attached")),
        "EnchantedController" | "EquippedController" => controller(json!("Attached")),
        "EnchantedPlayer" => Ok(json!("AttachedPlayer")),
        "ChosenCard" => ctx.chosen_card.as_ref().map_or_else(
            || unmapped("defined:ChosenCard-without-a-choice"),
            |name| Ok(json!({"Bound": name})),
        ),
        "DelayTriggerRemembered"
        | "DelayTriggerRememberedLKI"
        | "DelayedTriggerRemembered"
        | "RememberedLKI" => ctx
            .remembered
            .clone()
            .map_or_else(|| unmapped("defined:Remembered-without-a-change"), Ok),
        "Player.IsRemembered" | "Remembered" => ctx
            .remembered
            .clone()
            .map_or_else(|| unmapped("defined:Remembered-without-a-change"), Ok),
        "TriggeredCard"
        | "TriggeredPlayer"
        | "TriggeredAttacker"
        | "TriggeredBlocker"
        | "TriggeredCardLKICopy"
        | "TriggeredAttackerLKICopy"
        | "TriggeredNewCardLKICopy"
        | "TriggeredBlockerLKICopy" => Ok(json!("Triggering")),
        "TriggeredCardController" | "TriggeredSourceController" => controller(json!("Triggering")),
        "DefendingPlayer" | "TriggeredDefendingPlayer" => Ok(json!("Defending")),
        other => unmapped(format!("defined:{other}")),
    }
}

/// The reference when it names objects, not players: an effect on objects (pump, animate, gain control) refuses a player.
pub fn objects(reference: Value) -> Tr<Value> {
    let player = match &reference {
        Value::String(word) => matches!(
            word.as_str(),
            "You" | "Opponents" | "EachPlayer" | "AttachedPlayer"
        ),
        Value::Object(map) => map.contains_key("ControllerOf") || map.contains_key("OwnerOf"),
        _ => false,
    };
    if player {
        return unmapped("defined:player-for-an-object-effect");
    }
    Ok(reference)
}

/// The target specification `ValidTgts$` and its bounds stand for, as the DSL writes it.
pub fn target_spec(ctx: &Ctx, params: &Params, zone: Option<&str>) -> Tr<Value> {
    let valid = params.get("ValidTgts").unwrap_or_default();
    let what = target_what(ctx, valid, zone)?;
    let min = params.get("TargetMin");
    let max = params.get("TargetMax");
    let mut spec = json!({ "what": what });
    if min.is_some() || max.is_some() {
        let min = bound(ctx, min.unwrap_or("1"))?;
        let max = bound(ctx, max.unwrap_or("1"))?;
        if (&min, &max) != (&json!(1), &json!(1)) {
            spec["bounds"] = json!({"min": min, "max": max});
        }
    }
    if let Some(total) = params.get("DividedAsYouChoose") {
        spec["divide"] = amount(ctx, total)?;
    }
    Ok(spec)
}

fn bound(ctx: &Ctx, text: &str) -> Tr<Value> {
    if text == "Any" {
        return Ok(Value::Null);
    }
    amount(ctx, text)
}

fn target_what(ctx: &Ctx, valid: &str, zone: Option<&str>) -> Tr<Value> {
    let alternatives: Vec<&str> = valid.split(',').collect();
    let players = |word: &str| matches!(word, "Any" | "Player" | "Opponent" | "Player.Opponent");
    if alternatives.iter().all(|word| !players(word)) {
        return Ok(json!({"Object": selector_in(ctx, valid, zone)?}));
    }
    let mut kinds = Vec::new();
    for word in alternatives {
        kinds.push(match word {
            "Any" => json!("AnyTarget"),
            "Player" => json!("Player"),
            "Opponent" | "Player.Opponent" => json!("Opponent"),
            other => json!({"Object": selector_in(ctx, other, zone)?}),
        });
    }
    match kinds.as_slice() {
        [only] => Ok(only.clone()),
        _ => Ok(json!({ "Either": kinds })),
    }
}

/// The reference to what an ability line acts on: its target (`ValidTgts$`, declared on the context) or its
/// `Defined$` word, which is `default` when the line has none.
pub fn subject(ctx: &mut Ctx, params: &Params, default: &str, zone: Option<&str>) -> Tr<Value> {
    if params.has("ValidTgts") {
        let spec = target_spec(ctx, params, zone)?;
        return ctx.target(spec);
    }
    let word = params.get("Defined").unwrap_or(default);
    if word.is_empty() {
        let api = ["SP", "AB", "DB"]
            .iter()
            .find_map(|head| params.get(head))
            .unwrap_or("?");
        return unmapped(format!("defined:none-for-{api}"));
    }
    defined(ctx, word)
}
