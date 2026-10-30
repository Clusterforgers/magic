//! Repairing a proposal before it is judged: the slips of a model that the file itself and the printed card settle.
//!
//! A repair never decides what an ability means. It fixes a structure the file gets wrong in a way that has one
//! reading (an id the card gives, faces the model listed as abilities, a power the printed `N/N` states, a target
//! index shifted as a whole) or removes what carries no meaning (a `Manual` with an unusable span, an ability with
//! no effect). The read-back against the printed text stays the only judge, so a wrong repair ends as a mismatch.

mod controller;
mod declare;
mod effectless;
mod faces;
mod implied;
mod manual;
mod targets;
#[cfg(test)]
mod tests;
mod text;
mod tokens;
mod unused;
mod x_cost;

use std::fmt;

use serde_json::Value;

use crate::support::{PrintedCard, PrintedFace};

/// The class of slip a repair fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RepairClass {
    /// The oracle id or a name differs from the imported card's.
    Identity,
    /// More faces than the card has, or fewer.
    Faces,
    /// A `Manual` ability with an unusable span.
    ManualSpan,
    /// An ability with no effect.
    NoEffect,
    /// A creature token with only one of power and toughness.
    TokenSize,
    /// Target indexes shifted as a whole.
    TargetIndex,
    /// `Triggering` where the ability has a target no effect uses.
    TriggeringTarget,
    /// A target the effects use and the ability does not declare.
    TargetDeclared,
    /// A declared target no effect uses.
    TargetUnused,
    /// `ControllerOf` around a player where a player is meant.
    ControllerOfPlayer,
    /// An `X` where the card prints none.
    InventedX,
    /// A mana ability the basic land types of the face give.
    ImpliedMana,
}

impl RepairClass {
    /// The class as the words of the ledger.
    pub fn word(self) -> &'static str {
        match self {
            RepairClass::Identity => "identity from the card",
            RepairClass::Faces => "faces of the card",
            RepairClass::ManualSpan => "unusable Manual span",
            RepairClass::NoEffect => "ability without effects",
            RepairClass::TokenSize => "token size from the text",
            RepairClass::TargetIndex => "shifted target index",
            RepairClass::TriggeringTarget => "Triggering for the target",
            RepairClass::TargetDeclared => "target declared from the text",
            RepairClass::TargetUnused => "target no effect uses",
            RepairClass::ControllerOfPlayer => "controller of a player",
            RepairClass::InventedX => "X the card does not have",
            RepairClass::ImpliedMana => "mana the land types give",
        }
    }
}

/// The abilities that have effects and take targets.
pub(super) const EFFECT_ABILITIES: [&str; 4] = ["Spell", "Triggered", "Activated", "Loyalty"];

/// Where an ability stands in its face: what a repair reads the printed words of it with.
pub(super) struct Site<'a> {
    /// The printed face.
    pub face: &'a PrintedFace,
    /// How many abilities of the same kind come before it in the face.
    pub nth: usize,
    /// How many abilities of that kind the face has.
    pub count: usize,
    /// Where it is in the file, as a path.
    pub at: &'a str,
}

/// One thing the checker fixed in a proposal before judging it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repair {
    /// The class of slip.
    pub class: RepairClass,
    /// Where in the file, as a path (`faces[0].abilities[2]`).
    pub at: String,
    /// What was changed.
    pub note: String,
}

impl fmt::Display for Repair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.class.word(), self.at, self.note)
    }
}

/// Repairs the parsed JSON of a proposal in place and returns what it fixed.
pub(super) fn repair(value: &mut Value, printed: &PrintedCard) -> Vec<Repair> {
    let mut repairs = Vec::new();
    let Some(object) = value.as_object_mut() else {
        return repairs;
    };
    faces::repair(object, printed, &mut repairs);
    let Some(faces) = object.get_mut("faces").and_then(Value::as_array_mut) else {
        return repairs;
    };
    for (index, face) in faces.iter_mut().enumerate() {
        let Some(abilities) = face.get_mut("abilities").and_then(Value::as_array_mut) else {
            continue;
        };
        let shown = printed.faces.get(index);
        let text = shown.map_or("", |shown| shown.oracle_text.as_str());
        let at = format!("faces[{index}]");
        effectless::repair(abilities, &at, &mut repairs);
        if let Some(face) = shown {
            implied::repair(abilities, face, &at, &mut repairs);
        }
        manual::repair(abilities, text, &at, &mut repairs);
        let kinds: Vec<String> = abilities
            .iter_mut()
            .map(|ability| body_of(ability).map_or(String::new(), |(name, _)| name.to_owned()))
            .collect();
        for (n, ability) in abilities.iter_mut().enumerate() {
            let at = format!("{at}.abilities[{n}]");
            tokens::repair(ability, text, &at, &mut repairs);
            targets::repair(ability, &at, &mut repairs);
            controller::repair(ability, &at, &mut repairs);
            if let Some(face) = shown {
                let same = |name: &String| *name == kinds[n];
                let site = Site {
                    face,
                    nth: kinds[..n].iter().filter(|name| same(name)).count(),
                    count: kinds.iter().filter(|name| same(name)).count(),
                    at: &at,
                };
                declare::repair(ability, &site, &mut repairs);
                unused::repair(ability, &site, &mut repairs);
                x_cost::repair(ability, &site, &mut repairs);
            }
        }
    }
    repairs
}

/// The name of an ability and its body: `{"Spell": {...}}` is `("Spell", {...})`.
pub(super) fn body_of(ability: &mut Value) -> Option<(&str, &mut serde_json::Map<String, Value>)> {
    let map = ability.as_object_mut()?;
    if map.len() != 1 {
        return None;
    }
    let (name, body) = map.iter_mut().next()?;
    Some((name.as_str(), body.as_object_mut()?))
}
