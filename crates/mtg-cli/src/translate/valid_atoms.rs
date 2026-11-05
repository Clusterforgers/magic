//! Properties of a valid-string that the DSL's selector atoms stand for: how an object was cast, where it sits in a
//! library, who blocks it and what happened to it this turn.

use serde_json::{Value, json};

use super::ctx::{CARD_TYPES, Ctx, Tr, unmapped};
use super::valid::{defined, objects, selector_any};

/// The predicates of a property such as `wasCastFromYourHandByYou` or `blockingSource`, when it is one of these.
pub fn atom(prop: &str) -> Option<Vec<Value>> {
    let one = |value: Value| Some(vec![value]);
    match prop {
        "hasABasicLandType" => {
            let basic = ["Plains", "Island", "Swamp", "Mountain", "Forest"];
            return one(json!({"AnyOf": basic.map(|name| json!({"Subtype": name}))}));
        }
        "TopLibrary" => return one(json!("TopOfLibrary")),
        "BottomLibrary" => return one(json!("BottomOfLibrary")),
        "Suspended" => return one(json!("Suspended")),
        "ThisTurnEntered" => return one(json!("EnteredThisTurn")),
        "tokenCreated" => return one(json!("Token")),
        "wasDealtDamageThisTurn" => return one(json!("DamagedThisTurn")),
        "blockingSource" => return one(json!({"Blocks": "This"})),
        "blockedBySource" => return one(json!({"BlockedBy": "This"})),
        "attackingYou" => return one(json!({"AttackingPlayer": "You"})),
        "IsMonstrous" => return one(json!("Monstrous")),
        "IsRenowned" => return one(json!("Renowned")),
        "faceDown" => return one(json!("FaceDown")),
        "faceUp" => return one(json!({"Not": "FaceDown"})),
        "nonColorless" => return one(json!({"Not": "Colorless"})),
        // Only an attacker that no creature was declared to block; `blocked` is not its negation.
        "unblocked" => return one(json!("Unblocked")),
        // An attacker that was declared blocked (it stays blocked when its blockers are gone), so `Not Unblocked` while attacking.
        "blocked" => return Some(vec![json!("Attacking"), json!({"Not": "Unblocked"})]),
        "PairedWith" => return one(json!({"PairedWith": "This"})),
        "milledThisTurn" => return one(json!("MilledThisTurn")),
        "withTypeCycling" => return one(json!({"HasKeywordNamed": "Typecycling"})),
        "MonoColor" => return one(compare("Colors", "Eq", 1)),
        // How the spell was cast, for the ways the cast node has.
        "surged" => return one(json!({"CastWith": "Surge"})),
        "spectacle" => return one(json!({"CastWith": "Spectacle"})),
        "prowled" => return one(json!({"CastWith": "Prowl"})),
        "webSlinged" => return one(json!({"CastWith": "WebSlinging"})),
        _ => {}
    }
    if let Some(zone) = prop.strip_prefix("ThisTurnEnteredFrom_")
        && [
            "Battlefield",
            "Library",
            "Graveyard",
            "Hand",
            "Exile",
            "Stack",
        ]
        .contains(&zone)
    {
        return one(json!({"ArrivedFrom": zone}));
    }
    if let Some(found) = mana_cost(prop) {
        return Some(found);
    }
    if let Some(found) = counters(prop) {
        return Some(vec![found]);
    }
    if let Some(found) = counted(prop) {
        return Some(vec![found]);
    }
    was_cast(prop.strip_prefix("wasCast")?)
}

/// Whether a reference names players only (a word that does not is a gap, or an object whose controller is wanted).
pub(super) fn is_player(reference: &Value) -> bool {
    match reference {
        Value::String(word) => matches!(
            word.as_str(),
            "You" | "Opponents" | "EachPlayer" | "AttachedPlayer" | "Defending" | "ActivePlayer"
        ),
        Value::Object(map) => map.contains_key("ControllerOf") || map.contains_key("OwnerOf"),
        _ => false,
    }
}

/// The objects a word names: a valid-string (`Creature.YouCtrl`) as the cards that match it, else a `Defined$` word.
fn objects_of(ctx: &Ctx, word: &str) -> Tr<Value> {
    let word = word.trim();
    let head = word.split('.').next().unwrap_or(word);
    let base = CARD_TYPES.contains(&head)
        || matches!(head, "Permanent" | "Card")
        || ctx.world.subtypes.contains(head);
    if base {
        Ok(json!({"Each": selector_any(ctx, word)?}))
    } else {
        objects(defined(ctx, word)?)
    }
}

/// The player a `ControlledBy <word>` or `<word>Ctrl` names: the player a word holds, or the controller of the objects it
/// names (`Self`, `Enchanted`, `Valid ...`); `None` for a word that is neither.
fn controller_of(ctx: &Ctx, word: &str) -> Option<Value> {
    let word = word.trim();
    let found = defined(ctx, word).ok()?;
    if is_player(&found) {
        return Some(found);
    }
    let objects = word.starts_with("Valid ")
        || matches!(word, "Self" | "Card.Self" | "Enchanted" | "Equipped");
    objects.then(|| json!({ "ControllerOf": found }))
}

/// Properties that carry an argument or an object word: `AttachedTo <valid>`, `EnchantedBy <valid>`, `attacking <player>`,
/// `ControlledBy <word>`, `OwnedBy <word>`. `None` for any other property.
pub fn argument(ctx: &Ctx, prop: &str) -> Tr<Option<Vec<Value>>> {
    // Forge writes a comma in a name as `;`; only a name the checkout knows is a name (a bare `namedAjani` is a broken split).
    if let Some(name) = prop.strip_prefix("named")
        && !name.is_empty()
    {
        let name = name.replace(';', ",");
        return if ctx.world.names.contains(&name) {
            Ok(Some(vec![json!({"Named": name})]))
        } else {
            unmapped(format!("valid:named{name}"))
        };
    }
    let Some((head, arg)) = prop.split_once(' ') else {
        return Ok(None);
    };
    let one = |value: Value| Ok(Some(vec![value]));
    match head {
        // This attachment sits on one of the objects the word names.
        "AttachedTo" => one(json!({"AttachedTo": objects_of(ctx, arg)?})),
        // The object has an attachment that matches: the opposite direction of `AttachedTo`.
        "EnchantedBy" | "EquippedBy" => {
            let subtype = if head == "EnchantedBy" {
                "Aura"
            } else {
                "Equipment"
            };
            let mut attached = selector_any(ctx, arg)?;
            if !attached.contains(&json!({"Subtype": subtype})) {
                attached.push(json!({"Subtype": subtype}));
            }
            one(json!({ "HasAttached": attached }))
        }
        "attacking" => match defined(ctx, arg) {
            Ok(who) if is_player(&who) => one(json!({"AttackingPlayer": who})),
            _ => unmapped(format!("valid:attacking {arg}")),
        },
        "ControlledBy" | "OwnedBy" => {
            let Some(who) = controller_of(ctx, arg) else {
                return Ok(None);
            };
            let pred = if head == "ControlledBy" {
                "ControlledBy"
            } else {
                "OwnedBy"
            };
            one(json!({ pred: who }))
        }
        _ => Ok(None),
    }
}

/// `greatestPower`, `leastToughnessControlledByYou`, `greatestCMC`: the object has the most (or least) of the stat among the
/// objects of the selector's own base (`group`, e.g. creatures), narrowed to those a player controls when the word says so.
/// `None` for any other word; a base that names no kind of object, or a controller word unknown here, is a gap.
pub fn extreme(ctx: &Ctx, prop: &str, group: &[Value]) -> Tr<Option<Vec<Value>>> {
    let (pred, rest) = if let Some(rest) = prop.strip_prefix("greatest") {
        ("GreatestAmong", rest)
    } else if let Some(rest) = prop.strip_prefix("least") {
        ("LeastAmong", rest)
    } else {
        return Ok(None);
    };
    let (stat, rest) = [
        ("Power", "Power"),
        ("Toughness", "Toughness"),
        ("CMC", "ManaValue"),
    ]
    .iter()
    .find_map(|(word, stat)| rest.strip_prefix(word).map(|rest| (*stat, rest)))
    .map_or((None, rest), |(stat, rest)| (Some(stat), rest));
    let Some(stat) = stat else {
        return Ok(None);
    };
    let mut group = group.to_vec();
    if group.is_empty() {
        return unmapped(format!("valid:{prop}-without-a-kind"));
    }
    if !rest.is_empty() {
        let who = rest
            .strip_prefix("ControlledBy")
            .and_then(|word| match word {
                "Opponent" => Some(json!("Opponents")),
                other => controller_of(ctx, other),
            });
        let Some(who) = who else {
            return unmapped(format!("valid:{prop}"));
        };
        group.push(json!({"ControlledBy": who}));
    }
    Ok(Some(vec![json!({ pred: {"stat": stat, "group": group} })]))
}

/// `RememberedPlayerCtrl`, `EnchantedPlayerOwn`: the word names the player, the suffix says control or ownership.
pub fn suffixed(ctx: &Ctx, prop: &str) -> Option<Vec<Value>> {
    let (word, pred) = prop
        .strip_suffix("Ctrl")
        .map(|word| (word, "ControlledBy"))
        .or_else(|| prop.strip_suffix("Own").map(|word| (word, "OwnedBy")))?;
    let who = controller_of(ctx, word)?;
    Some(vec![json!({ pred: who })])
}

/// `counters_GE1_P1P1`, `counters_LT3_CHARGE`: the number of counters of a kind compared with a written number. Whether
/// there is one at all is `HasCounter` (or its negation); every other bound is `CounterCount`, where none counts as zero.
fn counters(prop: &str) -> Option<Value> {
    let rest = prop.strip_prefix("counters_")?;
    let (bound, kind) = rest.split_once('_')?;
    let op = super::valid::cmp(bound.get(..2)?)?;
    let digits = bound.get(2..)?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let to: u32 = digits.parse().ok()?;
    let kind = super::amount::counter_kind(kind)?;
    Some(match (op, to) {
        ("Ge", 1) | ("Gt" | "Ne", 0) => json!({"HasCounter": kind}),
        ("Eq" | "Le", 0) | ("Lt", 1) => json!({"Not": {"HasCounter": kind}}),
        _ => json!({"CounterCount": {"kind": kind, "op": op, "to": to}}),
    })
}

/// `numColorsEQ2`, `basePowerGE3`: a characteristic compared with a written number.
fn counted(prop: &str) -> Option<Value> {
    let (stat, rest) = [
        ("numColors", "Colors"),
        ("basePower", "BasePower"),
        ("baseToughness", "BaseToughness"),
    ]
    .iter()
    .find_map(|(word, stat)| prop.strip_prefix(word).map(|rest| (*stat, rest)))?;
    let op = super::valid::cmp(rest.get(..2)?)?;
    let digits = rest.get(2..)?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(compare(stat, op, digits.parse().ok()?))
}

/// `ManaCost0`, `ManaCost1`: a mana cost of exactly that many generic mana (`{1}`), not just that mana value: `{W}`,
/// `{U/B}` and `{B/P}` are other costs, and a land or a suspend-only card (Lotus Bloom) has no cost at all.
fn mana_cost(prop: &str) -> Option<Vec<Value>> {
    let number: i64 = prop.strip_prefix("ManaCost")?.parse().ok()?;
    Some(vec![json!({"GenericCost": number})])
}

fn compare(stat: &str, op: &str, to: i64) -> Value {
    json!({"Compare": {"stat": stat, "op": op, "to": to}})
}

/// `wasCast`, `wasCastByYou`, `wasCastFromYourHand`, `wasCastFromGraveyardByYou` and their kin.
fn was_cast(rest: &str) -> Option<Vec<Value>> {
    let (rest, by) = match rest.strip_suffix("ByYou") {
        Some(rest) => (rest, Some(json!("You"))),
        None => (rest, None),
    };
    let mut cast = serde_json::Map::new();
    if let Some(by) = by {
        cast.insert("by".to_owned(), by);
    }
    let mut owner = false;
    if let Some(zone) = rest.strip_prefix("From") {
        // A spell is cast from its caster's own hand, so `Their` adds no owner test.
        let (zone, yours) = match (zone.strip_prefix("Your"), zone.strip_prefix("Their")) {
            (Some(zone), _) => (zone, true),
            (None, Some(zone)) => (zone, false),
            (None, None) => (zone, false),
        };
        if !matches!(zone, "Hand" | "Graveyard" | "Exile" | "Library") {
            return None;
        }
        cast.insert("from".to_owned(), json!(zone));
        owner = yours;
    } else if !rest.is_empty() {
        return None;
    }
    let mut preds = vec![json!({ "WasCast": cast })];
    if owner {
        preds.push(json!({"OwnedBy": "You"}));
    }
    Some(preds)
}
