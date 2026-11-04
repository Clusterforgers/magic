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
        // Teams do not exist at our tables: a player's team is the player.
        "YouCtrl" | "YourTeamCtrl" => return one(json!({"ControlledBy": "You"})),
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
        "singleTarget" => return one(json!("SingleTarget")),
        "IsGoaded" | "Goaded" => return one(json!("Goaded")),
        "HasCounters" => return one(json!("HasAnyCounter")),
        "hasXCost" => return one(json!("XInCost")),
        "RememberedPlayerCtrl" => {
            return match super::memory::slot_ref(ctx, super::memory::Slot::Remembered) {
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
                None => super::memory::entered_with(ctx, prop).map(|pred| vec![pred]),
            };
        }
        "ExiledWithSource" => return one(json!({"ExiledWith": "This"})),
        "enchanted" => return one(json!({"HasAttached": [{"Subtype": "Aura"}]})),
        "equipped" => return one(json!({"HasAttached": [{"Subtype": "Equipment"}]})),
        "Historic" => {
            return one(json!({"AnyOf": [
                {"Supertype": "Legendary"}, {"Type": "Artifact"}, {"Subtype": "Saga"}]}));
        }
        _ => {}
    }
    if let Some(preds) = super::valid_atoms::argument(ctx, prop)? {
        return Ok(preds);
    }
    if let Some(preds) = super::valid_more::property(ctx, prop)? {
        return Ok(preds);
    }
    if let Some(preds) = super::valid_atoms::suffixed(ctx, prop) {
        return Ok(preds);
    }
    if let Some(preds) = super::valid_atoms::atom(prop) {
        return Ok(preds);
    }
    if let Some(preds) = typed_property(prop) {
        return Ok(preds);
    }
    if let Some(preds) = numeric_property(ctx, prop)? {
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
    // A keyword with a parameter (`withToxic`): has it at any value.
    if let Some(word) = prop.strip_prefix("with")
        && let Some(name) = mtg_cards::keyword::Keyword::NAMES
            .iter()
            .find(|name| name.eq_ignore_ascii_case(&spaced(word)))
    {
        return one(json!({"HasKeywordNamed": name}));
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
        if at > 0 && c.is_uppercase() && !out.ends_with(' ') {
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
fn numeric_property(ctx: &Ctx, prop: &str) -> Tr<Option<Vec<Value>>> {
    let Some((stat, rest)) = [
        ("power", "Power"),
        ("toughness", "Toughness"),
        ("cmc", "ManaValue"),
    ]
    .iter()
    .find_map(|(word, stat)| prop.strip_prefix(word).map(|rest| (*stat, rest))) else {
        return Ok(None);
    };
    let Some((op, bound)) = rest.get(..2).and_then(cmp).zip(rest.get(2..)) else {
        return Ok(None);
    };
    let named = bound
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if !named || !bound.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(None);
    }
    // The bound is a number or the part that names it (`cmcLEX` reads the part `X`).
    let to = amount(ctx, bound)?;
    Ok(Some(vec![
        json!({"Compare": {"stat": stat, "op": op, "to": to}}),
    ]))
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

/// The negation of the predicates of one property (`!token`); several predicates are negated by De Morgan: some of them fails.
fn negated(preds: Vec<Value>) -> Tr<Value> {
    let mut fails: Vec<Value> = preds
        .into_iter()
        .map(|pred| json!({ "Not": pred }))
        .collect();
    match fails.len() {
        0 => unmapped("valid:negation-of-nothing"),
        1 => Ok(fails.remove(0)),
        _ => Ok(json!({ "AnyOf": fails })),
    }
}

/// One alternative (`Creature.YouCtrl+Other`) as a selector.
pub fn selector(ctx: &Ctx, spec: &str) -> Tr<Vec<Value>> {
    let spec = spec.trim();
    let (head, props) = spec.split_once('.').unwrap_or((spec, ""));
    let mut preds = base(ctx, head)?;
    let kind = preds.clone();
    for prop in props.split('+').filter(|prop| !prop.is_empty()) {
        if let Some(found) = super::valid_atoms::extreme(ctx, prop, &kind)? {
            preds.extend(found);
            continue;
        }
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

/// A cost selector writes its alternatives with `;` (`Creature.Other;Artifact`), each with its own properties; a name keeps
/// its `;` (it stands for a comma).
fn commas(spec: &str) -> String {
    if spec.contains("named") {
        spec.to_owned()
    } else {
        spec.replace(';', ",")
    }
}

/// A comma-separated list of alternatives as a selector: one alternative, or alternatives that share their predicates but
/// one each (`Instant.YouCtrl,Sorcery.YouCtrl`, `Creature.attacking,Creature.blocking`).
pub fn selector_any(ctx: &Ctx, spec: &str) -> Tr<Vec<Value>> {
    let spec = &commas(spec);
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

/// A card outside the battlefield is its owner's: control in a valid-string becomes ownership.
fn owned_outside(preds: &mut [Value]) {
    for pred in preds {
        if let Some(who) = pred.get("ControlledBy").cloned() {
            *pred = json!({ "OwnedBy": who });
        }
    }
}

/// A selector over the cards of a zone: the alternatives with `In(zone)` added when the zone is not the battlefield.
pub fn selector_in(ctx: &Ctx, spec: &str, zone: Option<&str>) -> Tr<Vec<Value>> {
    let spec = &commas(spec);
    let mut preds = selector_any(ctx, spec)?;
    let permanents = spec
        .split(',')
        .filter(|alternative| alternative.trim().split('.').next() == Some("Permanent"))
        .count();
    let off_battlefield = !matches!(zone, None | Some("Battlefield"));
    if off_battlefield && permanents > 0 {
        // Outside the battlefield "permanent" is a card of a permanent type, not an instant or sorcery.
        if permanents != spec.split(',').count() {
            return unmapped("valid:Permanent-card-beside-other-alternatives");
        }
        let types = [
            "Artifact",
            "Battle",
            "Creature",
            "Enchantment",
            "Land",
            "Planeswalker",
        ];
        let any: Vec<Value> = types.iter().map(|name| json!({"Type": name})).collect();
        preds.push(json!({"AnyOf": any}));
    }
    match zone {
        None | Some("Battlefield") => {}
        Some(zone @ ("Graveyard" | "Hand" | "Library" | "Exile")) => {
            owned_outside(&mut preds);
            preds.push(json!({ "In": zone }));
        }
        // A list of zones (`Graveyard,Exile`): a card in any of them.
        Some(list) if list.split(',').count() > 1 => {
            let zones: Vec<&str> = list.split(',').collect();
            if !zones
                .iter()
                .all(|zone| matches!(*zone, "Graveyard" | "Hand" | "Exile"))
            {
                return unmapped(format!("zone:{list}"));
            }
            owned_outside(&mut preds);
            let any: Vec<Value> = zones.iter().map(|zone| json!({ "In": zone })).collect();
            preds.push(json!({ "AnyOf": any }));
        }
        Some(other) => return unmapped(format!("zone:{other}")),
    }
    Ok(preds)
}

/// The reference a `Defined$` word stands for.
pub fn defined(ctx: &Ctx, word: &str) -> Tr<Value> {
    if let Some(spec) = word.trim().strip_prefix("Valid ") {
        return Ok(json!({"Each": selector_any(ctx, spec)?}));
    }
    let controller = |inner: Value| Ok(json!({ "ControllerOf": inner }));
    match word.trim() {
        "You" => Ok(json!("You")),
        // Teams do not exist at our tables: the other players are the opponents.
        "Opponent" | "Player.Opponent" | "Player.Other" => Ok(json!("Opponents")),
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
        "ChosenCard" | "ChosenPlayer" => ctx.chosen_card.as_ref().map_or_else(
            || unmapped("defined:ChosenCard-without-a-choice"),
            |name| Ok(json!({"Bound": name})),
        ),
        "DelayTriggerRemembered"
        | "DelayTriggerRememberedLKI"
        | "DelayedTriggerRemembered"
        | "RememberedLKI"
        | "Player.IsRemembered"
        | "Remembered" => super::memory::remembered(ctx),
        word if super::triggered::is_word(word) => {
            super::triggered::resolve(ctx.trigger.as_ref(), word)
        }
        "DefendingPlayer" | "TriggeredDefendingPlayer" => Ok(json!("Defending")),
        other => super::valid_more::defined_word(ctx, other),
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
    // "Up to one target of each player" (`header::per_player_targets`).
    if let Some(rest) = valid.strip_prefix(super::header::FOR_EACH) {
        let Some((players, objects)) = rest.split_once('>') else {
            return unmapped("key:TargetsForEachPlayer=shape");
        };
        let what = selector_in(ctx, objects, zone)?;
        return Ok(json!({"PerPlayer": {"players": players, "what": what}}));
    }
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
