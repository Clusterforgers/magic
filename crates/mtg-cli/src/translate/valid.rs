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

/// `RememberedPlayerCtrl`: controlled by the remembered player. Cards a step moved are no player: asking whose they were
/// needs their controller as they left, so that stays a gap.
fn remembered_player_controls(ctx: &Ctx) -> Tr<Vec<Value>> {
    match super::memory::slot_ref(ctx, super::memory::Slot::Remembered) {
        Some(held)
            if held
                .get("Bound")
                .and_then(Value::as_str)
                .is_some_and(|name| ctx.counted.contains(name)) =>
        {
            unmapped("valid:RememberedPlayerCtrl-of-moved-cards")
        }
        Some(who) => Ok(vec![json!({"ControlledBy": who})]),
        None => unmapped("valid:RememberedPlayerCtrl-without-a-player"),
    }
}

/// `RememberedPlayerOwn`: owned by the remembered player, which must be a player: the one of a pass over players, or a
/// word that names players.
fn remembered_player_owns(ctx: &Ctx) -> Tr<Vec<Value>> {
    let held = super::memory::slot_ref(ctx, super::memory::Slot::Remembered);
    let pass = |held: &Value| {
        ctx.player_loop
            .as_ref()
            .is_some_and(|name| held == &json!({ "Bound": name }))
    };
    match held {
        Some(who) if pass(&who) || super::valid_atoms::is_player(&who) => {
            Ok(vec![json!({"OwnedBy": who})])
        }
        _ => unmapped("valid:RememberedPlayerOwn"),
    }
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
        "NotDefinedOriginalHost" if ctx.granted => return one(json!({"Not": {"Is": "Granter"}})),
        "Self" | "StrictlySelf" => return one(json!("This")),
        "EffectSource" if ctx.effect_source => return one(json!("This")),
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
        // Attached to the object the line is about: its source.
        "Attached" => return one(json!({"AttachedTo": "This"})),
        // A permanent card outside the battlefield; on the battlefield every object is one.
        "Permanent" => {
            return one(json!({"AnyOf": PERMANENT_TYPES.map(|name| json!({"Type": name}))}));
        }
        "modified" => return one(json!("Modified")),
        "singleTarget" => return one(json!("SingleTarget")),
        "IsGoaded" | "Goaded" => return one(json!("Goaded")),
        "HasCounters" => return one(json!("HasAnyCounter")),
        "hasXCost" => return one(json!("XInCost")),
        "RememberedPlayerOwn" => return remembered_player_owns(ctx),
        "RememberedPlayerCtrl" => {
            return remembered_player_controls(ctx);
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
    if let Some(pred) = keyword_named(prop) {
        return one(pred);
    }
    if let Some(rest) = prop.strip_prefix("non")
        && ctx.world.subtypes.contains(&capitalize(rest))
    {
        return one(json!({"Not": {"Subtype": capitalize(rest)}}));
    }
    if prop.chars().next().is_some_and(char::is_uppercase) && ctx.world.subtypes.contains(prop) {
        return one(json!({"Subtype": prop}));
    }
    super::filter::atom(ctx, ctx.scope.get(), prop)
        .unwrap_or_else(|| unmapped(format!("valid:{prop}")))
}

/// A keyword with a parameter (`withToxic`, `withoutForetell`): has it, or lacks it, at any value.
fn keyword_named(prop: &str) -> Option<Value> {
    let named = |word: &str| {
        mtg_cards::keyword::Keyword::NAMES
            .iter()
            .find(|name| name.eq_ignore_ascii_case(&spaced(word)))
    };
    if let Some(name) = prop.strip_prefix("without").and_then(named) {
        return Some(json!({"Not": {"HasKeywordNamed": name}}));
    }
    let name = prop.strip_prefix("with").and_then(named)?;
    Some(json!({ "HasKeywordNamed": name }))
}

/// `FirstStrike` as `First Strike`.
pub(super) fn spaced(word: &str) -> String {
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

pub(super) fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `powerGE4`, `toughnessLE2`, `cmcEQ3`.
fn numeric_property(ctx: &Ctx, prop: &str) -> Tr<Option<Vec<Value>>> {
    // `Spell.ManaSpent GE4`: the mana spent to cast the spell against a number. A named bound (`LTX`) is read against the
    // card in some scripts and the spell in others, so only a number is read.
    if let Some(rest) = prop.strip_prefix("ManaSpent ")
        && let Some((op, bound)) = rest.get(..2).and_then(cmp).zip(rest.get(2..))
        && let Ok(bound) = bound.parse::<u32>()
    {
        return Ok(Some(vec![
            json!({"Compare": {"stat": "ManaSpent", "op": op, "to": bound}}),
        ]));
    }
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
        _ => super::filter::base(word).map_or_else(|| unmapped(format!("valid-base:{word}")), Ok),
    }
}

/// The negation of the predicates of one property (`!token`); several predicates are negated by De Morgan: some of them fails.
fn negated(preds: Vec<Value>) -> Tr<Value> {
    // `!CastSaSource`: not the source itself is `Other`.
    if preds == [json!({"Not": "Other"})] {
        return Ok(json!("Other"));
    }
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

/// The properties of an alternative, split at `+`. A property with an object argument (`AttachedTo Creature.attacking`)
/// takes the `+` parts after it: they belong to the object it names (`AttachedTo Creature.attacking+OppCtrl` is an Aura
/// on an attacking creature an opponent controls).
pub(super) fn split_properties(props: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut greedy = false;
    for part in props.split('+').filter(|part| !part.is_empty()) {
        match out.last_mut() {
            Some(last) if greedy => {
                last.push('+');
                last.push_str(part);
            }
            _ => {
                greedy = ["AttachedTo ", "EnchantedBy ", "EquippedBy "]
                    .iter()
                    .any(|head| part.strip_prefix(head).is_some_and(|arg| arg.contains('.')));
                out.push(part.to_owned());
            }
        }
    }
    out
}

/// One alternative (`Creature.YouCtrl+Other`) as a selector.
pub fn selector(ctx: &Ctx, spec: &str) -> Tr<Vec<Value>> {
    let spec = spec.trim();
    let (head, props) = spec.split_once('.').unwrap_or((spec, ""));
    // A player is no object: the property names what the engine lacks.
    if head == "Player" && !props.is_empty() {
        return unmapped(format!("valid:Player.{props}"));
    }
    // Every unknown word of the alternative is reported, not only the first.
    let mut gaps = super::filter::Unknowns::default();
    let (mut preds, props) = if matches!(head, "Targeted" | "TargetedCard") {
        targeted_base(&objects(ctx.last_target()?)?, props)
    } else if let Some(slot) = memory_head(head) {
        targeted_base(&objects(super::memory::held_objects(ctx, slot)?)?, props)
    } else {
        (gaps.take(base(ctx, head)), split_properties(props))
    };
    let kind = preds.clone();
    for prop in props {
        let prop = prop.as_str();
        match super::valid_atoms::extreme(ctx, prop, &kind) {
            Ok(Some(found)) => {
                preds.extend(found);
                continue;
            }
            Ok(None) => {}
            Err(gap) => {
                gaps.note(&gap);
                continue;
            }
        }
        match prop.strip_prefix('!') {
            Some(inner) => match property(ctx, inner).and_then(negated) {
                Ok(pred) => preds.push(pred),
                Err(gap) => gaps.note(&gap),
            },
            None => preds.extend(gaps.take(property(ctx, prop))),
        }
    }
    gaps.finish()?;
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

/// The card types a permanent can have.
const PERMANENT_TYPES: [&str; 6] = [
    "Artifact",
    "Battle",
    "Creature",
    "Enchantment",
    "Land",
    "Planeswalker",
];

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

/// `ValidSource$` as a selector. Forge writes "a source" as `Card.YouCtrl,Emblem.YouCtrl` because an emblem is not a card;
/// the selector of a source already takes any object, so an emblem alternative that a card alternative with the same
/// properties covers (`Other` aside: an emblem is never the object itself) is dropped.
pub fn source_any(ctx: &Ctx, spec: &str) -> Tr<Vec<Value>> {
    let spec = commas(spec);
    let alternatives: Vec<&str> = spec.split(',').map(str::trim).collect();
    let props = |alternative: &str, base: &str| -> Option<Vec<String>> {
        let rest = alternative.strip_prefix(base)?;
        if !rest.is_empty() && !rest.starts_with('.') {
            return None;
        }
        let mut props: Vec<String> = rest
            .split('+')
            .map(|part| part.trim_start_matches('.').to_owned())
            .filter(|part| !part.is_empty() && part != "Other")
            .collect();
        props.sort();
        Some(props)
    };
    let kept: Vec<&str> = alternatives
        .iter()
        .copied()
        .filter(|alternative| {
            props(alternative, "Emblem").is_none_or(|emblem| {
                !alternatives
                    .iter()
                    .any(|other| props(other, "Card").as_ref() == Some(&emblem))
            })
        })
        .collect();
    let before = ctx.scope.replace(super::filter::Scope::Source);
    let found = selector_any(ctx, &kept.join(","));
    ctx.scope.set(before);
    found
}

/// A comma-separated list of alternatives as a selector: one alternative, or alternatives that share their predicates but
/// one each (`Instant.YouCtrl,Sorcery.YouCtrl`, `Creature.attacking,Creature.blocking`).
pub fn selector_any(ctx: &Ctx, spec: &str) -> Tr<Vec<Value>> {
    let spec = &commas(spec);
    let alternatives: Vec<&str> = spec.split(',').collect();
    if let [only] = alternatives.as_slice() {
        return selector(ctx, only);
    }
    if let Err(gap) = super::filter::parse(spec) {
        return unmapped(gap);
    }
    let mut gaps = super::filter::Unknowns::default();
    let mut lists: Vec<Vec<Value>> = Vec::new();
    for alternative in &alternatives {
        match selector(ctx, alternative) {
            Ok(list) => lists.push(list),
            Err(gap) => gaps.note(&gap),
        }
    }
    gaps.finish()?;
    let common: Vec<Value> = lists[0]
        .iter()
        .filter(|pred| lists.iter().all(|list| list.contains(pred)))
        .cloned()
        .collect();
    let mut different = Vec::new();
    // An alternative with no zone beside alternatives that name another zone (`TgtZone$ Stack,Battlefield`) is the table.
    let zoned = lists.iter().any(|list| {
        list.iter()
            .any(|pred| pred.get("In").is_some() && !common.contains(pred))
    });
    for list in &lists {
        let rest: Vec<&Value> = list.iter().filter(|pred| !common.contains(pred)).collect();
        match rest.as_slice() {
            [one] => different.push((*one).clone()),
            [] if zoned && !common.iter().any(|pred| pred.get("In").is_some()) => {
                different.push(json!({"In": "Battlefield"}));
            }
            // An alternative with only the shared predicates holds the others: the union is the shared predicates.
            [] => return Ok(common),
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
    let mut out: Vec<Value> = lists[0]
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
        .collect();
    if !placed {
        out.push(any);
    }
    Ok(out)
}

/// A card outside the battlefield is its owner's: control in a valid-string becomes ownership.
fn owned_outside(preds: &mut [Value]) {
    for pred in preds {
        if let Some(who) = pred.get("ControlledBy").cloned() {
            *pred = json!({ "OwnedBy": who });
        }
    }
}

/// For a selector of cards outside the battlefield: "permanent" is a card of a permanent type, not an instant or sorcery.
/// `None` when no alternative says permanent; a mixture with other alternatives is a gap.
pub fn permanent_card(spec: &str) -> Tr<Option<Value>> {
    let permanents = spec
        .split(',')
        .filter(|alternative| alternative.trim().split('.').next() == Some("Permanent"))
        .count();
    if permanents == 0 {
        return Ok(None);
    }
    let types = [
        "Artifact",
        "Battle",
        "Creature",
        "Enchantment",
        "Land",
        "Planeswalker",
    ];
    // A permanent type beside "permanent" is inside it.
    let inside = spec
        .split(',')
        .filter(|alternative| {
            let base = alternative.trim().split('.').next().unwrap_or_default();
            types.contains(&base)
        })
        .count();
    if permanents + inside != spec.split(',').count() {
        return unmapped("valid:Permanent-card-beside-other-alternatives");
    }
    let any: Vec<Value> = types.iter().map(|name| json!({"Type": name})).collect();
    Ok(Some(json!({"AnyOf": any})))
}

/// "Creature card of the chosen type" printed where the script says any card of it (`Card.ChosenType`): a Kindred card
/// of that type is no creature card, so the creature type joins the predicates.
fn fix9_creature_of_chosen_type(ctx: &Ctx, spec: &str, preds: &mut Vec<Value>) {
    let printed = [
        "creature card of the chosen type",
        "creature cards of the chosen type",
    ]
    .iter()
    .any(|words| ctx.oracle.contains(words));
    let untyped = !preds.iter().any(|pred| pred.get("Type").is_some());
    if printed && untyped && spec.split(',').count() == 1 && spec.starts_with("Card.ChosenType") {
        preds.insert(0, json!({"Type": "Creature"}));
    }
}

/// Adds "a card of a permanent type" to the predicates of a `Permanent` spec that names no card type of its own.
pub fn fix9_permanent_card(spec: &str, preds: &mut Vec<Value>) -> Tr<()> {
    let permanent = permanent_card(spec)?;
    let is_type = |pred: &Value| pred.get("Type").is_some();
    let typed = preds.iter().any(|pred| {
        is_type(pred)
            || pred
                .get("AnyOf")
                .and_then(Value::as_array)
                .is_some_and(|any| any.iter().all(is_type))
    });
    if !typed {
        preds.extend(permanent);
    }
    Ok(())
}

/// "Put there this turn" outside the battlefield: `EnteredThisTurn` tallies only arrivals on the battlefield, so a card in
/// another zone is selected by its latest arrival from any zone.
fn arrived_outside(preds: &mut [Value]) {
    for pred in preds.iter_mut().filter(|pred| *pred == "EnteredThisTurn") {
        let origins = [
            "Battlefield",
            "Hand",
            "Library",
            "Graveyard",
            "Stack",
            "Exile",
            "Command",
        ];
        *pred = json!({"AnyOf": origins.map(|zone| json!({"ArrivedFrom": zone}))});
    }
}

/// A selector over the cards of a zone: the alternatives with `In(zone)` added when the zone is not the battlefield.
pub fn selector_in(ctx: &Ctx, spec: &str, zone: Option<&str>) -> Tr<Vec<Value>> {
    let spec = &commas(spec);
    let mut preds = selector_any(ctx, spec)?;
    fix9_creature_of_chosen_type(ctx, spec, &mut preds);
    if !matches!(zone, None | Some("Battlefield"))
        && let Some(any) = permanent_card(spec)?
    {
        preds.push(any);
    }
    if !matches!(zone, None | Some("Battlefield")) {
        arrived_outside(&mut preds);
    }
    match zone {
        None | Some("Battlefield") => {}
        Some(zone @ ("Graveyard" | "Hand" | "Library" | "Exile" | "Command")) => {
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
    // `ValidGraveyard Creature.YouOwn`: the objects of a zone list, read as `Count$Valid<zones>` reads them.
    if let Some(rest) = word.trim().strip_prefix("Valid")
        && let Some((zones, spec)) = rest.split_once(' ')
        && !zones.is_empty()
        && zones.chars().all(|c| c.is_ascii_alphabetic() || c == ',')
    {
        let zones: Vec<&str> = zones.split(',').collect();
        return Ok(json!({"Each": super::count::zone_selector(ctx, &zones, spec)?}));
    }
    // `Targeted.Creature`: the target when it is a creature ("if it's a creature, it can't block this turn").
    if let Some(kind) = word.trim().strip_prefix("Targeted.")
        && matches!(
            kind,
            "Creature" | "Artifact" | "Enchantment" | "Land" | "Planeswalker"
        )
    {
        return Ok(json!({"Each": [{"Is": ctx.last_target()?}, {"Type": kind}]}));
    }
    let controller = |inner: Value| Ok(json!({ "ControllerOf": inner }));
    match word.trim() {
        "You" => Ok(json!("You")),
        // Teams do not exist at our tables: the other players are the opponents.
        "Opponent" | "Player.Opponent" | "Player.Other" => Ok(json!("Opponents")),
        "Player" => Ok(json!("EachPlayer")),
        "Self" | "Card.Self" => Ok(json!("This")),
        "EffectSource" | "Card.EffectSource" if ctx.effect_source => Ok(json!("This")),
        "Sacrificed" | "Discarded" => Ok(json!("Paid")),
        "Convoked" => Ok(json!("Convoked")),
        "Exiled" => super::cost::exiled_paid(ctx, "defined"),
        // The cards this permanent exiled and that are still in exile.
        "ExiledWith" => Ok(json!({"Each": [{"ExiledWith": "This"}, {"In": "Exile"}]})),
        "TopOfLibrary" => {
            Ok(json!({"Each": [{"In": "Library"}, {"OwnedBy": "You"}, "TopOfLibrary"]}))
        }
        "Targeted" | "TargetedPlayer" | "TargetedCard" | "ThisTargetedCard" => ctx.last_target(),
        "ParentTarget" => ctx.parent(),
        "TargetedController" => controller(ctx.last_target()?),
        "TargetedOrController" => Ok(json!({"OrController": ctx.last_target()?})),
        "TargetedOwner" => Ok(json!({ "OwnerOf": ctx.last_target()? })),
        "RememberedOwner" => Ok(json!({ "OwnerOf": super::memory::remembered(ctx)? })),
        "Enchanted" | "Equipped" => Ok(json!("Attached")),
        "OriginalHost" if ctx.granted => Ok(json!("Granter")),
        "EnchantedController" | "EquippedController" => controller(json!("Attached")),
        "EnchantedPlayer" | "Player.EnchantedBy" => Ok(json!("AttachedPlayer")),
        "ChosenCard" => ctx.chosen_card.as_ref().map_or_else(
            || unmapped("defined:ChosenCard-without-a-choice"),
            |name| Ok(json!({"Bound": name})),
        ),
        "ChosenPlayer" => chosen_player(ctx, "defined:ChosenCard-without-a-choice"),
        "Player.Chosen" => chosen_player(ctx, "defined:Player.Chosen"),
        "RememberedController" => super::memory::remembered_controller(ctx),
        "ImprintedController" => super::memory::imprinted_controller(ctx),
        "DelayTriggerRemembered" | "DelayTriggerRememberedLKI" | "DelayedTriggerRemembered"
            if ctx.delayed_remembered.is_some() =>
        {
            Ok(ctx.delayed_remembered.clone().unwrap_or_default())
        }
        "DelayTriggerRemembered"
        | "DelayTriggerRememberedLKI"
        | "DelayedTriggerRemembered"
        | "RememberedLKI"
        | "RememberedCard"
        | "Remembered" => super::memory::remembered(ctx),
        "Player.IsRemembered" => {
            let held = super::memory::remembered(ctx)?;
            if super::memory::mixes_the_player(ctx, &held) {
                return unmapped("defined:Player.IsRemembered-after-a-remembered-change");
            }
            Ok(held)
        }
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
    // A target being chosen reads no other target of its line (`filter::Scope::Targeting`).
    let before = ctx.scope.replace(super::filter::Scope::Targeting);
    let what = target_what(ctx, valid, zone);
    ctx.scope.set(before);
    let mut what = what?;
    // Outside the battlefield the controller is the owner ("from your graveyard"), which the text states in its own words.
    if matches!(zone, None | Some("Battlefield"))
        && super::any_player::narrows_target(ctx.oracle, valid)
    {
        return unmapped("target:script-narrower-than-oracle");
    }
    if let Some(word) = params.get("TargetsWithDefinedController") {
        restrict_to_controller(ctx, word, zone, &mut what)?;
    }
    for (key, rule) in [
        ("TargetsWithSameController", "SameController"),
        ("TargetsWithDifferentControllers", "DifferentControllers"),
        ("TargetsWithDifferentNames", "DifferentNames"),
        ("TargetsWithDifferentCMC", "DifferentManaValues"),
    ] {
        match params.get(key) {
            None | Some("False") => {}
            Some("True") if what.get("Object").is_some() => {
                what = json!({"Together": {"what": what, "rule": rule}});
            }
            Some(other) => return unmapped(format!("key:{key}={other}")),
        }
    }
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
    if valid.starts_with(super::header::FOR_EACH) {
        // `per_player_targets` took the "up to one each" bounds off the line; the engine reads min 0 as up to one of each.
        spec["bounds"] = json!({"min": 0, "max": 1});
    }
    if let Some(total) = params.get("DividedAsYouChoose") {
        spec["divide"] = amount(ctx, total)?;
    }
    Ok(spec)
}

/// `TargetsWithDefinedController$`: the targets are objects that one player controls (owns, outside the battlefield).
fn restrict_to_controller(ctx: &Ctx, word: &str, zone: Option<&str>, what: &mut Value) -> Tr<()> {
    let Some(preds) = what.get_mut("Object").and_then(Value::as_array_mut) else {
        return unmapped("key:TargetsWithDefinedController-for-players");
    };
    // An earlier target of the same spell is not yet chosen when the targets are offered: the engine cannot read it in a filter.
    let who = if matches!(
        word.trim(),
        "TriggeredCardController"
            | "TriggeredTarget"
            | "TriggeredDefendingPlayer"
            | "TriggeredPlayer"
            | "TriggeredSourceController"
    ) {
        defined(ctx, word)?
    } else {
        return unmapped(format!("key:TargetsWithDefinedController={word}"));
    };
    let key = if matches!(zone, None | Some("Battlefield")) {
        "ControlledBy"
    } else {
        "OwnedBy"
    };
    preds.push(json!({ key: who }));
    Ok(())
}

fn bound(ctx: &Ctx, text: &str) -> Tr<Value> {
    if text == "Any" {
        return Ok(Value::Null);
    }
    amount(ctx, text)
}

/// "Target spell or nonland permanent": alternatives that name another zone (`Card.inZoneStack`) beside alternatives of the
/// battlefield. Each alternative is a whole conjunction with its own zone, so the engine looks in every zone named.
fn mixed_zones(ctx: &Ctx, alternatives: &[&str]) -> Tr<Option<Vec<Value>>> {
    fn elsewhere(word: &str) -> bool {
        word.contains("inZoneStack")
    }
    // Another zone beside the stack reads ownership, which the plain selector handles.
    if alternatives
        .iter()
        .any(|word| word.contains("inZone") && !elsewhere(word))
    {
        return Ok(None);
    }
    let outside = alternatives.iter().filter(|word| elsewhere(word)).count();
    if outside == 0 || outside == alternatives.len() {
        return Ok(None);
    }
    let mut parts = Vec::new();
    for word in alternatives {
        let mut preds = selector_any(ctx, word)?;
        if !elsewhere(word) {
            preds.push(json!({"In": "Battlefield"}));
        }
        parts.push(super::count::all_of(preds));
    }
    Ok(Some(vec![json!({"AnyOf": parts})]))
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
        if matches!(zone, None | Some("Battlefield"))
            && let Some(preds) = mixed_zones(ctx, &alternatives)?
        {
            return Ok(json!({"Object": preds}));
        }
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

/// The memory a base word of a valid-string reads (`Remembered.sameName`, `Imprinted.Creature`).
fn memory_head(head: &str) -> Option<super::memory::Slot> {
    match head {
        "Remembered" | "RememberedCard" => Some(super::memory::Slot::Remembered),
        "Imprinted" => Some(super::memory::Slot::Imprinted),
        _ => None,
    }
}

/// The base `Targeted` (or a memory) of a valid-string: the objects themselves, the cards that share a name with one of
/// them (`sameName`, which leaves them out beside `Other`) or what is attached to them (`Attached`); the properties left
/// apply to those objects.
fn targeted_base(target: &Value, props: &str) -> (Vec<Value>, Vec<String>) {
    let mut rest = split_properties(props);
    let mut take = |word: &str| {
        let found = rest.iter().any(|prop| prop == word);
        rest.retain(|prop| prop != word);
        found
    };
    let (same_name, attached, other) = (take("sameName"), take("Attached"), take("Other"));
    take("Self");
    let mut preds = Vec::new();
    if same_name {
        preds.push(json!({"SharesName": target}));
        if other {
            preds.push(json!({"Not": {"Is": target}}));
        }
    } else if attached {
        preds.push(json!({"AttachedTo": target}));
        if other {
            preds.push(json!("Other"));
        }
    } else {
        preds.push(json!({"Is": target}));
        if other {
            preds.push(json!("Other"));
        }
    }
    (preds, rest)
}

/// The player the line or the source chose: a name an earlier step bound, else the one the source chose as it entered.
pub(super) fn chosen_player(ctx: &Ctx, gap: &str) -> Tr<Value> {
    // `ChoosePlayer` binds `player…`; a card or a source chosen later is no player.
    if let Some(name) = &ctx.chosen_card {
        return if name.starts_with("player") {
            Ok(json!({"Bound": name}))
        } else {
            unmapped(gap)
        };
    }
    if ctx.memory.entered.player {
        return Ok(json!("Chosen"));
    }
    unmapped(gap)
}
