//! The ability header: trigger words the header grammar reads, and the keys every API shares (the zone of a target).

use serde_json::{Value, json};

use super::condition::comparison;
use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The trigger pattern of a `Mode$` word read here; `None` until a word is read.
pub fn trigger_words(_ctx: &Ctx, _p: &Params, _mode: &str) -> Option<Tr<Value>> {
    None
}

/// The keys of [`gate_conditions`].
pub const GATE_KEYS: [&str; 6] = [
    "ConditionPlayerTurn",
    "ConditionManaSpent",
    "ConditionManaNotSpent",
    "ConditionLifeTotal",
    "ConditionLifeAmount",
    "ConditionNotPresent",
];

/// The mana type a color letter names.
fn mana_type(letter: &str) -> Option<&'static str> {
    Some(match letter {
        "W" => "White",
        "U" => "Blue",
        "B" => "Black",
        "R" => "Red",
        "G" => "Green",
        "C" => "Colorless",
        _ => return None,
    })
}

/// The conditions the gate keys of a chain line state: whose turn it is, which mana was spent on the spell, and how the life
/// totals compare (`ConditionNotPresent$` is read by [`super::condition::prefixed`]).
pub fn gate_conditions(ctx: &Ctx, p: &Params) -> Tr<Vec<Value>> {
    let mut all = Vec::new();
    match p.get("ConditionPlayerTurn") {
        None => {}
        Some("True") => all.push(json!("YourTurn")),
        Some("False") => all.push(json!({"Not": "YourTurn"})),
        Some(other) => return unmapped(format!("condition:PlayerTurn={other}")),
    }
    if let Some(text) = p.get("ConditionManaSpent") {
        let mut spent = Vec::new();
        for letter in text.split_whitespace() {
            let Some(kind) = mana_type(letter) else {
                return unmapped(format!("condition:ManaSpent={letter}"));
            };
            spent.push(json!({ "ManaSpent": kind }));
        }
        match spent.len() {
            0 => return unmapped("condition:ManaSpent="),
            1 => all.append(&mut spent),
            _ => all.push(json!({ "All": spent })),
        }
    }
    if let Some(text) = p.get("ConditionManaNotSpent") {
        let Some(kind) = mana_type(text) else {
            return unmapped(format!("condition:ManaNotSpent={text}"));
        };
        all.push(json!({"Not": {"ManaSpent": kind}}));
    }
    match (p.get("ConditionLifeTotal"), p.get("ConditionLifeAmount")) {
        (None, None) => {}
        (Some("You"), Some(compare)) => {
            let (op, right) = comparison(ctx, compare)?;
            all.push(json!({"Compare": {"left": {"Life": "You"}, "op": op, "right": right}}));
        }
        (Some(other), _) => return unmapped(format!("condition:LifeTotal={other}")),
        (None, Some(_)) => return unmapped("condition:LifeAmountWithoutLifeTotal"),
    }
    Ok(all)
}

/// The turn structure, in order, as Forge names a phase and the DSL names its step.
const PHASES: [(&str, &str); 13] = [
    ("Untap", "Untap"),
    ("Upkeep", "Upkeep"),
    ("Draw", "Draw"),
    ("Main1", "Main1"),
    ("BeginCombat", "BeginCombat"),
    ("Declare Attackers", "DeclareAttackers"),
    ("Declare Blockers", "DeclareBlockers"),
    ("First Strike Damage", "FirstStrikeDamage"),
    ("Combat Damage", "CombatDamage"),
    ("EndCombat", "EndCombat"),
    ("Main2", "Main2"),
    ("End of Turn", "End"),
    ("Cleanup", "Cleanup"),
];

/// The place of a phase word in the turn.
fn phase_index(word: &str) -> Option<usize> {
    PHASES.iter().position(|(name, _)| *name == word.trim())
}

/// The condition that the turn is in one of the phases `ActivationPhases$` lists (`Upkeep`, `BeginCombat->EndCombat`, or
/// several joined with commas); any turn, as the word says nothing of whose it is.
fn in_phases(text: &str) -> Tr<Value> {
    let mut steps: Vec<usize> = Vec::new();
    let mut whole_combat = false;
    for part in text.split(',') {
        let (from, to) = part.split_once("->").unwrap_or((part, part));
        let (Some(from), Some(to)) = (phase_index(from), phase_index(to)) else {
            return unmapped(format!("key:ActivationPhases={text}"));
        };
        if from > to {
            return unmapped(format!("key:ActivationPhases={text}"));
        }
        whole_combat |= (from, to) == (4, 9);
        steps.extend(from..=to);
    }
    steps.sort_unstable();
    steps.dedup();
    if whole_combat && steps == [4, 5, 6, 7, 8, 9] {
        return Ok(json!("InCombat"));
    }
    let named: Vec<Value> = steps
        .iter()
        .map(|index| json!({ "Step": PHASES[*index].1 }))
        .collect();
    Ok(match named.as_slice() {
        [only] => only.clone(),
        _ => json!({ "Any": named }),
    })
}

/// What the words about when an ability can be activated state.
#[derive(Debug, Default)]
pub struct Activation {
    /// What must hold as it is activated.
    pub conditions: Vec<Value>,
    /// How often, when a word limits it.
    pub once_per_turn: bool,
    /// The zone it is activated from, when a word moves it.
    pub from: Option<&'static str>,
}

/// The keys of [`activation_words`].
pub const ACTIVATION_WORDS: [&str; 3] = ["ActivationPhases", "Boast", "Forecast"];

/// The phases of an activation (`ActivationPhases$`), boast ("only if this creature attacked this turn and only once each
/// turn") and forecast ("from your hand, during your upkeep, once each turn").
pub fn activation_words(p: &Params) -> Tr<Activation> {
    let mut out = Activation::default();
    if let Some(text) = p.get("ActivationPhases") {
        out.conditions.push(in_phases(text)?);
    }
    match p.get("Boast") {
        None => {}
        Some("True") => {
            out.conditions
                .push(json!({"Matches": {"subject": "This", "selector": ["AttackedThisTurn"]}}));
            out.once_per_turn = true;
        }
        Some(other) => return unmapped(format!("key:Boast={other}")),
    }
    match p.get("Forecast") {
        None => {}
        Some("True") => {
            out.conditions.push(json!("YourTurn"));
            out.conditions.push(json!({"Step": "Upkeep"}));
            out.once_per_turn = true;
            out.from = Some("Hand");
        }
        Some(other) => return unmapped(format!("key:Forecast={other}")),
    }
    Ok(out)
}

/// The gating words of a trigger line (`PlayerTurn$ True`, `Delirium$ True`).
const TRIGGER_GATES: [&str; 10] = [
    "PlayerTurn",
    "OpponentTurn",
    "Delirium",
    "Threshold",
    "Hellbent",
    "Metalcraft",
    "FirstCombat",
    "Revolt",
    "WerewolfTransformCondition",
    "WerewolfUntransformCondition",
];

/// Whether the line is a trigger (a `Mode$` with an `Execute$`), whose gating words are conditions of the event.
fn is_trigger(p: &Params) -> bool {
    peek(p, "Mode").is_some() && peek(p, "Execute").is_some()
}

/// The gating words a line of this kind has, to be removed with its condition keys.
pub fn gate_words(p: &Params) -> &'static [&'static str] {
    if is_trigger(p) { &TRIGGER_GATES } else { &[] }
}

/// The conditions the gating words of a trigger line state. A turn word is a condition of the event ("during your turn",
/// CR 603.4 does not recheck it); an "if" word is rechecked as the ability resolves unless the line says otherwise.
pub fn trigger_gates(p: &Params) -> Tr<Vec<Value>> {
    let mut all = Vec::new();
    for word in gate_words(p) {
        let Some(value) = p.get(word) else { continue };
        if value != "True" {
            return unmapped(format!("condition:{word}={value}"));
        }
        all.push(match *word {
            "PlayerTurn" => json!({"AtTrigger": "YourTurn"}),
            "OpponentTurn" => json!({"AtTrigger": {"Not": "YourTurn"}}),
            "FirstCombat" => json!("FirstCombat"),
            "Revolt" => super::condition::revolt(),
            "WerewolfTransformCondition" => super::condition::spells_last_turn(false),
            "WerewolfUntransformCondition" => super::condition::spells_last_turn(true),
            other => super::condition::named(other)?,
        });
    }
    Ok(all)
}

/// The line with some pairs replaced by new values and some dropped; the order of the keys stays.
fn rebuilt(p: &Params, replace: &[(&str, String)], drop: &[&str]) -> Params {
    let body: Vec<String> = p
        .iter()
        .filter(|(key, _)| !drop.contains(key))
        .map(|(key, value)| {
            let value = replace
                .iter()
                .find(|(name, _)| *name == key)
                .map_or(value, |(_, new)| new.as_str());
            format!("{key}$ {value}")
        })
        .collect();
    Params::parse(&body.join(" | "))
}

/// The value of a key without counting it as read.
fn peek<'a>(p: &'a Params, key: &str) -> Option<&'a str> {
    p.iter().find(|(name, _)| *name == key).map(|(_, v)| v)
}

/// The zones `TgtZone$` names, `None` for a zone the DSL's selectors do not know.
fn target_zones(text: &str) -> Option<Vec<&str>> {
    let zones: Vec<&str> = text.split(',').collect();
    zones
        .iter()
        .all(|zone| {
            matches!(
                *zone,
                "Battlefield" | "Graveyard" | "Hand" | "Library" | "Exile" | "Stack"
            )
        })
        .then_some(zones)
}

/// One alternative of `ValidTgts$` for the cards of `zone`: the zone as a property, and control as ownership outside the
/// stack and the battlefield. `None` for a player word, a `Permanent` (a card of a permanent type off the battlefield)
/// and a control property that ownership cannot stand for.
fn in_zone(alternative: &str, zone: &str, extra: Option<&str>) -> Option<String> {
    let (base, props) = alternative.split_once('.').unwrap_or((alternative, ""));
    if matches!(base, "Any" | "Player" | "Opponent" | "Permanent") {
        return None;
    }
    let owned = !matches!(zone, "Battlefield" | "Stack");
    let mut all: Vec<String> = Vec::new();
    for prop in props
        .split('+')
        .chain(extra)
        .filter(|prop| !prop.is_empty())
    {
        all.push(match prop {
            "YouCtrl" if owned => "YouOwn".to_owned(),
            "OppCtrl" if owned => "OppOwn".to_owned(),
            _ if owned && prop.contains("Ctrl") => return None,
            other => other.to_owned(),
        });
    }
    if zone != "Battlefield" {
        all.push(format!("inZone{zone}"));
    }
    Some(format!("{base}.{}", all.join("+")))
}

/// `TgtZone$` (and `TargetType$ Spell` beside `TgtZone$ Stack`) folded into the target's valid-string as `inZone` properties,
/// so the mapper of the verb sees a target of those cards. `None` leaves the line as it is: a zone the mapper reads itself
/// (`Origin$` equal to the zone; `Counter` and the copies of spells read `TargetType$`), or a target the zone cannot state.
pub fn zone_targets(p: &Params) -> Option<Params> {
    let zone_text = peek(p, "TgtZone")?;
    let valid = peek(p, "ValidTgts")?;
    let zones = target_zones(zone_text)?;
    let api = ["SP", "AB", "DB"].iter().find_map(|head| peek(p, head))?;
    if matches!(api, "Counter" | "CopySpellAbility" | "ChangeTargets") {
        return None;
    }
    if let Some(origin) = peek(p, "Origin") {
        return (origin == zone_text && peek(p, "TargetType").is_none())
            .then(|| rebuilt(p, &[], &["TgtZone"]));
    }
    let extra = match peek(p, "TargetType") {
        None => None,
        Some("Spell") if zone_text == "Stack" => None,
        Some("Spell.OppCtrl") if zone_text == "Stack" => Some("OppCtrl"),
        Some("Spell.YouCtrl") if zone_text == "Stack" => Some("YouCtrl"),
        Some(_) => return None,
    };
    let mut out = Vec::new();
    for alternative in valid.split(',') {
        for zone in &zones {
            out.push(in_zone(alternative.trim(), zone, extra)?);
        }
    }
    Some(rebuilt(
        p,
        &[("ValidTgts", out.join(","))],
        &["TgtZone", "TargetType"],
    ))
}

/// The prefix `ValidTgts$` gets from [`per_player_targets`]: the players, then `>`, then the valid-string without them.
pub const FOR_EACH: &str = "ForEach ";

/// `TargetsForEachPlayer$ True` with "up to one each" bounds (`TargetMin$ 0`, `TargetMax$ OneEach`): at most one target of
/// each player. The control word of every alternative of `ValidTgts$` names the players (`OppCtrl` the opponents, none
/// each player); the line gets the prefix [`FOR_EACH`] and loses the three keys. Exactly one each, other bounds and a
/// mixture of control words stay gaps.
pub fn per_player_targets(p: &Params) -> Tr<Option<Params>> {
    match peek(p, "TargetsForEachPlayer") {
        None => return Ok(None),
        Some("True") => {}
        Some(other) => return unmapped(format!("key:TargetsForEachPlayer={other}")),
    }
    if (peek(p, "TargetMin"), peek(p, "TargetMax")) != (Some("0"), Some("OneEach")) {
        return unmapped("key:TargetsForEachPlayer=bounds");
    }
    let Some(valid) = peek(p, "ValidTgts") else {
        return unmapped("key:TargetsForEachPlayer=without-targets");
    };
    let (mut opponents, mut anyone) = (0, 0);
    let mut stripped = Vec::new();
    for alternative in valid.split(',') {
        let (base, props) = alternative.split_once('.').unwrap_or((alternative, ""));
        let mut kept = Vec::new();
        let mut counted = false;
        for prop in props.split('+').filter(|prop| !prop.is_empty()) {
            if prop == "OppCtrl" {
                opponents += 1;
                counted = true;
            } else if prop.contains("Ctrl") || prop.contains("Own") {
                return unmapped("key:TargetsForEachPlayer=control");
            } else {
                kept.push(prop);
            }
        }
        anyone += i32::from(!counted);
        stripped.push(if kept.is_empty() {
            base.to_owned()
        } else {
            format!("{base}.{}", kept.join("+"))
        });
    }
    let players = match (opponents, anyone) {
        (0, _) => "EachPlayer",
        (_, 0) => "Opponents",
        _ => return unmapped("key:TargetsForEachPlayer=mixed-control"),
    };
    Ok(Some(rebuilt(
        p,
        &[(
            "ValidTgts",
            format!("{FOR_EACH}{players}>{}", stripped.join(",")),
        )],
        &["TargetsForEachPlayer", "TargetMin", "TargetMax"],
    )))
}

/// The kinds of things a declared target can be, from its `what`: `(objects, players)`.
fn target_kinds(spec: &Value) -> (bool, bool) {
    match &spec["what"] {
        Value::Object(map) if map.contains_key("Object") => (true, false),
        Value::String(word) if word == "Player" || word == "Opponent" => (false, true),
        _ => (true, true),
    }
}

/// `TargetUnique$ True`: the targets of the line are none of those the earlier lines of the chain declared (CR 115.3 lets
/// one object be chosen for two targets unless the card says "another"). Each object alternative of `ValidTgts$` gains a
/// `NotTarget<n>` property for every earlier target that can be an object; a player alternative after a target that can
/// be a player is not stated, so the line stays a gap. `None` leaves the line as it is.
pub fn unique_targets(ctx: &Ctx, p: &Params) -> Tr<Option<Params>> {
    match peek(p, "TargetUnique") {
        None => return Ok(None),
        Some("True") => {}
        Some("False") => return Ok(Some(rebuilt(p, &[], &["TargetUnique"]))),
        Some(other) => return unmapped(format!("key:TargetUnique={other}")),
    }
    let kinds: Vec<(bool, bool)> = ctx.targets.iter().map(target_kinds).collect();
    let valid = peek(p, "ValidTgts").filter(|_| !kinds.is_empty());
    let Some(valid) = valid else {
        return Ok(Some(rebuilt(p, &[], &["TargetUnique"])));
    };
    let nots: Vec<String> = kinds
        .iter()
        .enumerate()
        .filter(|(_, (objects, _))| *objects)
        .map(|(at, _)| format!("NotTarget{at}"))
        .collect();
    let mut out = Vec::new();
    for alternative in valid.split(',') {
        let base = alternative.split_once('.').map_or(alternative, |(b, _)| b);
        if matches!(base.trim(), "Any" | "Player" | "Opponent") {
            if kinds.iter().any(|(_, players)| *players) {
                return unmapped("key:TargetUnique=players");
            }
            // "Any other target": a player, or a creature, planeswalker or battle that is none of the earlier targets.
            if base.trim() == "Any" && alternative.trim() == "Any" && !nots.is_empty() {
                let others: Vec<String> = ["Creature", "Planeswalker", "Battle"]
                    .iter()
                    .map(|kind| format!("{kind}.{}", nots.join("+")))
                    .collect();
                out.push(format!("Player,{}", others.join(",")));
            } else {
                out.push(alternative.to_owned());
            }
        } else if nots.is_empty() {
            out.push(alternative.to_owned());
        } else {
            let join = if alternative.contains('.') { "+" } else { "." };
            out.push(format!("{}{join}{}", alternative.trim(), nots.join("+")));
        }
    }
    Ok(Some(rebuilt(
        p,
        &[("ValidTgts", out.join(","))],
        &["TargetUnique"],
    )))
}
