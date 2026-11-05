//! The objects a trigger's `Defined$` words (`TriggeredCard`, `TriggeredBlocker`, ...) name, read against what the
//! engine holds for the trigger's mode: `Triggering` is the event's subjects, `Dealer` the object that did it.

use serde_json::{Value, json};

use super::ctx::{Tr, unmapped};
use super::triggers_did::holds;

/// What a trigger line is about, known once its pattern is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    /// The Forge mode.
    pub mode: String,
    /// The pattern is about the card itself ("when this creature dies").
    pub itself: bool,
    /// The pattern names the object that did it (`ValidBlocker$` of a block), so the event holds a `Dealer`.
    pub by: bool,
    /// The event names a player as its target (`ValidTarget$ Player`), so `TriggeredTarget` is that player.
    pub to_player: bool,
}

/// The kind of object a word names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Word {
    Card,
    Attacker,
    Blocker,
    Player,
    /// The owner of the triggering card.
    Owner,
    /// The player who cast the spell or activated the ability.
    Activator,
    Source,
    Target,
}

/// The word's kind and whether it names that object's controller; none for a word this module does not know.
fn kind(word: &str) -> Option<(Word, bool)> {
    Some(match word {
        "TriggeredCard" | "TriggeredCardLKICopy" | "TriggeredNewCardLKICopy" => (Word::Card, false),
        "TriggeredCardController" => (Word::Card, true),
        "TriggeredCardOwner" => (Word::Owner, true),
        "TriggeredActivator" => (Word::Activator, false),
        "TriggeredAttacker" | "TriggeredAttackerLKICopy" => (Word::Attacker, false),
        "TriggeredAttackerController" => (Word::Attacker, true),
        "TriggeredBlocker" | "TriggeredBlockerLKICopy" => (Word::Blocker, false),
        "TriggeredBlockerController" => (Word::Blocker, true),
        "TriggeredTargetController" => (Word::Target, true),
        "TriggeredPlayer" => (Word::Player, false),
        "TriggeredSource" | "TriggeredSourceLKICopy" => (Word::Source, false),
        "TriggeredSourceController" => (Word::Source, true),
        "TriggeredTarget" | "TriggeredTargetLKICopy" => (Word::Target, false),
        _ => return None,
    })
}

/// Whether a word is one this module reads.
pub fn is_word(word: &str) -> bool {
    kind(word.trim()).is_some()
}

/// Modes whose event holds the one object it is about and no player.
const ONE_OBJECT: [&str; 7] = [
    "ChangesZone",
    "Taps",
    "Untaps",
    "TurnFaceUp",
    "CounterAdded",
    "CounterRemoved",
    "Attacks",
];

/// Modes whose event holds a player and one object.
const PLAYER_AND_OBJECT: [&str; 4] = ["Discarded", "Cycled", "Sacrificed", "SpellCast"];

/// Modes whose event holds one player and no object.
const ONE_PLAYER: [&str; 4] = ["LifeGained", "LifeLost", "Drawn", "Phase"];

/// Modes whose event object keeps its incarnation, so a name taken from it still reaches the object later.
const STAYS: [&str; 8] = [
    "Attacks",
    "AttackerBlocked",
    "AttackerBlockedByCreature",
    "Blocks",
    "Taps",
    "Untaps",
    "DamageDone",
    "BecomesTarget",
];

/// The reference a `RememberObjects$ Triggered*` word of a line that sets up a later trigger or effect names. A word for the
/// object after a zone change (`TriggeredNewCard`) needs the object one move on, and an object that moved is another one
/// (CR 400.7), so those stay gaps.
pub fn remembered_by_event(shape: Option<&Shape>, word: &str) -> Tr<Value> {
    let stays = shape.is_some_and(|shape| STAYS.contains(&shape.mode.as_str()));
    let object = !matches!(kind(word.trim()), Some((Word::Player | Word::Activator, _)));
    if word.contains("NewCard") || word.ends_with("Controller") || (object && !stays) {
        return unmapped(format!("key:RememberObjects={}-after-a-move", word.trim()));
    }
    resolve(shape, word)
}

/// The reference a `Triggered*` word names in a trigger of this shape, or the gap when the engine does not hold it.
pub fn resolve(shape: Option<&Shape>, word: &str) -> Tr<Value> {
    let gap = || {
        let mode = shape.map_or("no-trigger", |shape| shape.mode.as_str());
        unmapped(format!("defined:{word}-in-{mode}"))
    };
    let (Some(shape), Some((what, controller))) = (shape, kind(word.trim())) else {
        return gap();
    };
    let mode = shape.mode.as_str();
    let did = holds(mode);
    let reference = match what {
        Word::Owner if ONE_OBJECT.contains(&mode) => "Triggering",
        // An activation holds the player and the permanent; the player alone is "that player".
        Word::Activator if mode == "AbilityCast" => "TriggeringPlayer",
        Word::Activator if matches!(mode, "SpellCast" | "SpellAbilityCast") => "Triggering",
        Word::Card if ONE_OBJECT.contains(&mode) || PLAYER_AND_OBJECT.contains(&mode) => {
            "Triggering"
        }
        Word::Card if did.is_some_and(|(_, object)| object) => "Triggering",
        Word::Attacker
            if matches!(
                mode,
                "Attacks" | "AttackerBlocked" | "AttackerBlockedByCreature"
            ) =>
        {
            "Triggering"
        }
        Word::Blocker if mode == "AttackerBlockedByCreature" && shape.by => "Dealer",
        // A block holds both creatures; only "this blocks" names one without doubt.
        Word::Blocker if mode == "Blocks" && shape.itself => "This",
        Word::Player if did == Some((true, true)) => "TriggeringPlayer",
        Word::Player
            if ONE_PLAYER.contains(&mode)
                || PLAYER_AND_OBJECT.contains(&mode)
                || did.is_some_and(|(player, _)| player) =>
        {
            "Triggering"
        }
        Word::Source if mode == "DamageDone" => "Dealer",
        Word::Target if mode == "DamageDone" => "Triggering",
        _ => return gap(),
    };
    let object = matches!(what, Word::Card | Word::Attacker | Word::Owner);
    let reference = if shape.itself && object && reference == "Triggering" {
        "This"
    } else {
        reference
    };
    if !controller {
        return Ok(json!(reference));
    }
    match (what, reference) {
        (Word::Owner, _) => Ok(json!({"OwnerOf": reference})),
        (Word::Card, "This") if ONE_OBJECT.contains(&mode) => Ok(json!("You")),
        (Word::Card, "Triggering") if ONE_OBJECT.contains(&mode) => {
            Ok(json!({"ControllerOf": "Triggering"}))
        }
        (Word::Source, "Dealer") => Ok(json!({"ControllerOf": "Dealer"})),
        // The caster controls the spell on the stack; the creature hit or attacking is a permanent.
        (Word::Card, "Triggering") if mode == "SpellCast" => {
            Ok(json!({"ControllerOf": "Triggering"}))
        }
        (Word::Attacker, "Triggering") => Ok(json!({"ControllerOf": "Triggering"})),
        (Word::Attacker, "This") => Ok(json!("You")),
        (Word::Blocker, "Dealer") => Ok(json!({"ControllerOf": "Dealer"})),
        (Word::Blocker, "This") => Ok(json!("You")),
        (Word::Target, "Triggering") if !shape.to_player => {
            Ok(json!({"ControllerOf": "Triggering"}))
        }
        _ => gap(),
    }
}
