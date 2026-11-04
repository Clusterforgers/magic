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
    /// The names of the checkout's cards and tokens.
    pub names: BTreeSet<String>,
    /// The slugs of the cards the database stores, when the run reads it: a name a card conjures is checked against them.
    pub stored: Option<BTreeSet<String>>,
}

impl World {
    /// The subtypes of the card scripts and the given token scripts.
    pub fn new(scripts: &[Script], tokens: BTreeMap<String, Face>) -> World {
        let mut subtypes = BTreeSet::new();
        let mut names = BTreeSet::new();
        for face in scripts.iter().flat_map(|script| &script.faces) {
            subtypes.extend(subtypes_of(&face.types));
            names.insert(face.name.clone());
        }
        for face in tokens.values() {
            subtypes.extend(subtypes_of(&face.types));
            names.insert(face.name.clone());
        }
        World {
            subtypes,
            tokens,
            names,
            stored: None,
        }
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
    /// The `PT:` line of the face.
    pub pt: Option<&'a str>,
    /// The checkout's subtypes and tokens.
    pub world: &'a World,
    /// The target specifications of the ability being translated, in order.
    pub targets: Vec<Value>,
    /// The objects the latest zone change moved (`RememberChanged$`), which `Defined$ Remembered` names later.
    pub remembered: Option<Value>,
    /// The objects the latest `Imprint*` step moved, which `Defined$ Imprinted` names later.
    pub imprinted: Option<Value>,
    /// For a name a step bound to what it moved, the reference the step named them by: they are other objects now (CR 400.7)
    /// and `RememberedController` is read on those named before the move.
    pub moved_from: BTreeMap<String, Value>,
    /// The bound names whose step keeps how many objects it moved (`<name>~moved`), which survives a token that ceased to exist.
    pub counted: std::collections::BTreeSet<String>,
    /// The memories the card reads on a line other than the one that wrote them.
    pub memory: super::memory::Plan,
    /// The objects an `Effect` object remembers, which a trigger or static it carries reads as `IsRemembered`; kept across
    /// `reset` because the carried lines are translated as lines of their own.
    pub pinned: Option<Value>,
    /// What `DelayTriggerRemembered` names in the steps of a delayed trigger: the name its setup bound.
    pub delayed_remembered: Option<Value>,
    /// The index of the target the line being translated is about to declare, for the condition it states ahead of its effect.
    pub coming: Option<u8>,
    /// The latest target the ability had declared before the line being translated (`Defined$ ParentTarget`).
    pub parent_target: Option<u8>,
    /// The name the latest value chosen by the ability is bound to (`ChosenColor`, `ChosenType` after it).
    pub chosen: Option<String>,
    /// The name the cards the latest `ChooseCard` picked are bound to (`ChosenCard` after it).
    pub chosen_card: Option<String>,
    /// The name the latest number chosen by the ability is bound to (`ChosenNumber` after it).
    pub chosen_number: Option<String>,
    /// What the trigger line being translated is about, for its `Triggered*` words.
    pub trigger: Option<super::triggered::Shape>,
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
            pt: face.pt.as_deref(),
            world,
            targets: Vec::new(),
            remembered: None,
            imprinted: None,
            moved_from: BTreeMap::new(),
            counted: std::collections::BTreeSet::new(),
            coming: None,
            memory: super::memory::plan(face),
            pinned: None,
            delayed_remembered: None,
            parent_target: None,
            chosen: None,
            chosen_card: None,
            chosen_number: None,
            trigger: None,
            binds: 0,
            depth: 0,
        }
    }

    /// Starts the next ability: no targets and no bound names yet.
    pub fn reset(&mut self) {
        self.targets.clear();
        self.remembered = None;
        self.imprinted = None;
        self.moved_from.clear();
        self.counted.clear();
        self.coming = None;
        self.parent_target = None;
        self.chosen = None;
        self.chosen_card = None;
        self.chosen_number = None;
        self.trigger = None;
        self.binds = 0;
        self.depth = 0;
    }

    /// Runs a translation of a line of its own (a static, a trigger) and restores the state of the ability around it.
    pub fn nested<T>(&mut self, pinned: Option<Value>, run: impl FnOnce(&mut Ctx<'a>) -> T) -> T {
        let before = std::mem::replace(&mut self.pinned, pinned);
        let targets = std::mem::take(&mut self.targets);
        let saved = (
            self.remembered.take(),
            self.imprinted.take(),
            std::mem::take(&mut self.moved_from),
            self.parent_target,
            self.chosen.take(),
            self.chosen_card.take(),
            self.chosen_number.take(),
            self.trigger.take(),
            self.binds,
            self.depth,
        );
        let out = run(self);
        self.pinned = before;
        self.targets = targets;
        (
            self.remembered,
            self.imprinted,
            self.moved_from,
            self.parent_target,
            self.chosen,
            self.chosen_card,
            self.chosen_number,
            self.trigger,
            self.binds,
            self.depth,
        ) = saved;
        out
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
        if let Some(index) = self.coming {
            return Ok(json!({ "Target": index }));
        }
        match u8::try_from(self.targets.len().saturating_sub(1)) {
            Ok(index) if !self.targets.is_empty() => Ok(json!({ "Target": index })),
            _ => unmapped("defined:Targeted-without-target"),
        }
    }

    /// The reference to the latest declared target when it is a player (`TargetedPlayer`), else a gap.
    pub fn player_target(&self) -> Tr<Value> {
        let player = self
            .targets
            .last()
            .is_some_and(|spec| matches!(spec["what"].as_str(), Some("Player" | "Opponent")));
        if player {
            self.last_target()
        } else {
            unmapped("defined:TargetedPlayer-without-a-player-target")
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
    check_keys(params, api, read, |key, _| COMMON.contains(&key))
}

/// The one place that checks the keys of a line against those a mapper reads: a key that is neither read, accepted, nor a
/// hint makes the line a gap `key:<label>.<key>`, and a listed key that the mapper then never reads makes it a gap too
/// (`audit` in `card.rs`).
pub fn check_keys(
    params: &Params,
    label: &str,
    read: &[&str],
    accepted: impl Fn(&str, &str) -> bool,
) -> Tr<()> {
    for (key, value) in params.iter() {
        if read.contains(&key) || accepted(key, value) || is_hint(key) {
            continue;
        }
        return unmapped(format!("key:{label}.{key}"));
    }
    let promised: Vec<&str> = read
        .iter()
        .copied()
        .filter(|key| !is_hint(key) && !INERT.contains(&(label, *key)))
        .collect();
    params.promise(label, &promised);
    Ok(())
}

/// Keys that a mapper lists and may leave unread because they change nothing the translation keeps: bookkeeping of
/// remembered objects, the hidden-zone flags of a library search (it may find nothing and always shuffles), and display
/// flags of a private look.
const INERT: [(&str, &str); 14] = [
    ("Cleanup", "ClearRemembered"),
    ("Cleanup", "ClearImprinted"),
    ("Cleanup", "ClearTriggered"),
    ("Cleanup", "ClearChosenCard"),
    ("ChooseSource", "ChoiceTitle"),
    ("Effect", "Stackable"),
    ("ChangeZone.Search", "Hidden"),
    ("ChangeZone.Search", "Mandatory"),
    ("ChangeZone.Search", "Optional"),
    ("ChangeZone.Search", "ShuffleNonMandatory"),
    ("ChangeZone.Choose", "Hidden"),
    ("ChangeZone.Choose", "Optional"),
    ("Static.Continuous", "AddSVar"),
    ("Dig", "ForceRevealToController"),
];
