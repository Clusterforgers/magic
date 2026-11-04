//! The state of one translation: the named parts of the face, what the checkout knows (subtypes, token scripts), the
//! targets declared so far and the names of bound results, and the error that names an unmapped Forge construct.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::forge::script::{Face, Params, Script};

/// A Forge construct the translator has no mapper for, named for the ranking of gaps (`effect:Dig`, `key:Pump.Duration`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unmapped(pub String);

/// The result of translating one construct.
pub type Tr<T> = Result<T, Unmapped>;

/// An error that names the construct.
pub fn unmapped<T>(name: impl Into<String>) -> Tr<T> {
    Err(Unmapped(name.into()))
}

/// What the checkout knows besides the card scripts: the subtypes its type lines name and its token scripts.
#[derive(Debug, Clone, Default)]
pub struct World {
    /// Every word after the card types of a type line, so that a capitalized word in a valid-string can be told from a
    /// property.
    pub subtypes: BTreeSet<String>,
    /// The token scripts by file stem (`w_1_1_soldier`).
    pub tokens: BTreeMap<String, Face>,
}

impl World {
    /// The subtypes of the card scripts and the given token scripts.
    pub fn new(scripts: &[Script], tokens: BTreeMap<String, Face>) -> World {
        let mut subtypes = BTreeSet::new();
        for face in scripts.iter().flat_map(|script| &script.faces) {
            subtypes.extend(subtypes_of(&face.types));
        }
        for face in tokens.values() {
            subtypes.extend(subtypes_of(&face.types));
        }
        World { subtypes, tokens }
    }
}

/// The card types a `Types:` line may start with.
pub const CARD_TYPES: [&str; 11] = [
    "Artifact",
    "Battle",
    "Creature",
    "Enchantment",
    "Instant",
    "Kindred",
    "Land",
    "Planeswalker",
    "Sorcery",
    "Tribal",
    "Conspiracy",
];

/// The supertypes a `Types:` line may start with.
pub const SUPERTYPES: [&str; 4] = ["Basic", "Legendary", "Snow", "World"];

/// The subtypes a type line names: its words that are neither card types nor supertypes.
pub fn subtypes_of(types: &str) -> Vec<String> {
    types
        .split_whitespace()
        .filter(|word| !CARD_TYPES.contains(word) && !SUPERTYPES.contains(word))
        .map(str::to_owned)
        .collect()
}

/// The translation of one face.
pub struct Ctx<'a> {
    /// The named parts of the face.
    pub svars: &'a BTreeMap<String, String>,
    /// The `Types:` line of the face.
    pub types: &'a str,
    /// Forge's rendering of the card's text, only to tell two spellings of the same rule apart.
    pub oracle: &'a str,
    /// The `ManaCost:` line of the face.
    pub mana_cost: &'a str,
    /// The checkout's subtypes and tokens.
    pub world: &'a World,
    /// The target specifications of the ability being translated, in order.
    pub targets: Vec<Value>,
    /// The objects the latest zone change moved (`RememberChanged$`), which `Defined$ Remembered` names later.
    pub remembered: Option<Value>,
    /// The latest target the ability had declared before the line being translated (`Defined$ ParentTarget`).
    pub parent_target: Option<u8>,
    /// The name the latest value chosen by the ability is bound to (`ChosenColor`, `ChosenType` after it).
    pub chosen: Option<String>,
    /// The name the cards the latest `ChooseCard` picked are bound to (`ChosenCard` after it).
    pub chosen_card: Option<String>,
    binds: usize,
    depth: usize,
}

/// The deepest chain of named parts followed before the translator gives up (a cycle in a script).
const MAX_DEPTH: usize = 24;

impl<'a> Ctx<'a> {
    /// A context for a face.
    pub fn new(face: &'a Face, world: &'a World) -> Ctx<'a> {
        Ctx {
            svars: &face.svars,
            types: &face.types,
            oracle: &face.oracle,
            mana_cost: &face.mana_cost,
            world,
            targets: Vec::new(),
            remembered: None,
            parent_target: None,
            chosen: None,
            chosen_card: None,
            binds: 0,
            depth: 0,
        }
    }

    /// Starts the next ability: no targets and no bound names yet.
    pub fn reset(&mut self) {
        self.targets.clear();
        self.remembered = None;
        self.parent_target = None;
        self.chosen = None;
        self.chosen_card = None;
        self.binds = 0;
        self.depth = 0;
    }

    /// The text of a named part.
    pub fn svar(&self, name: &str) -> Option<&'a str> {
        self.svars.get(name.trim()).map(String::as_str)
    }

    /// The parts of the named part (an ability line), or the gap that names a missing or looping reference.
    pub fn sub(&mut self, name: &str) -> Tr<Params> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return unmapped("svar:loop");
        }
        match self.svar(name) {
            Some(text) => Ok(Params::parse(text)),
            None => unmapped("svar:missing"),
        }
    }

    /// Declares a target and returns the reference to its chosen set.
    pub fn target(&mut self, spec: Value) -> Tr<Value> {
        let at = self.targets.len();
        let Ok(index) = u8::try_from(at) else {
            return unmapped("targets:too-many");
        };
        self.targets.push(spec);
        Ok(json!({ "Target": index }))
    }

    /// The reference to the latest declared target ("Targeted" in a sub-ability).
    pub fn last_target(&self) -> Tr<Value> {
        match u8::try_from(self.targets.len().saturating_sub(1)) {
            Ok(index) if !self.targets.is_empty() => Ok(json!({ "Target": index })),
            _ => unmapped("defined:Targeted-without-target"),
        }
    }

    /// The reference to the target declared before the current line (`ParentTarget` in a sub-ability).
    pub fn parent(&self) -> Tr<Value> {
        match self.parent_target {
            Some(index) => Ok(json!({ "Target": index })),
            None => unmapped("defined:ParentTarget-without-target"),
        }
    }

    /// Notes the latest target declared so far as the parent target of the lines that follow.
    pub fn mark_parent(&mut self) {
        self.parent_target = u8::try_from(self.targets.len().saturating_sub(1))
            .ok()
            .filter(|_| !self.targets.is_empty());
    }

    /// A name for a result bound by one step and read by a later one.
    pub fn fresh(&mut self, base: &str) -> String {
        self.binds += 1;
        if self.binds == 1 {
            base.to_owned()
        } else {
            format!("{base}{}", self.binds)
        }
    }

    /// Whether the face is an Aura.
    pub fn is_aura(&self) -> bool {
        self.types.split_whitespace().any(|word| word == "Aura")
    }
}

/// The keys every ability line may carry besides its verb's own: text for people, hints for Forge's computer player and
/// the keys the chain reads.
const COMMON: [&str; 11] = [
    "SP",
    "AB",
    "DB",
    "Cost",
    "SubAbility",
    "SorcerySpeed",
    "PlayerTurn",
    "ActivationLimit",
    "ActivationZone",
    "Planeswalker",
    "Ultimate",
];

/// Whether a key is only text for people or a hint for Forge's computer player.
pub fn is_hint(key: &str) -> bool {
    const SUFFIXES: [&str; 4] = ["Description", "Desc", "Prompt", "Title"];
    const EXACT: [&str; 12] = [
        "Image",
        "AILogic",
        "AIPreference",
        "StackDescription",
        "SpellDescription",
        "PrecostDesc",
        "CostDesc",
        "NonBasicSpell",
        "ShowCurrentCard",
        "Secondary",
        "IsCurse",
        "TriggerDescription",
    ];
    EXACT.contains(&key)
        || SUFFIXES.iter().any(|suffix| key.ends_with(suffix))
        || (key.starts_with("AI") && key.len() > 2)
}

/// Refuses a key that the mapper of `api` does not read: a key that changes what the line does and is not understood
/// must leave the line out, not be ignored.
pub fn only(params: &Params, api: &str, read: &[&str]) -> Tr<()> {
    for (key, _) in params.iter() {
        if read.contains(&key) || COMMON.contains(&key) || is_hint(key) {
            continue;
        }
        return unmapped(format!("key:{api}.{key}"));
    }
    Ok(())
}
