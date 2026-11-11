//! rmt2: permanents of a target player held down through their next untap step ("Lands target player controls don't
//! untap during their next untap step", "Creatures target player controls don't untap during that player's next untap
//! step").

use super::Writer;
use crate::refs::{Pred, Ref, Selector, Targetable};

/// The plural name of each card type the selector names besides the controller, in order: "creatures and lands".
fn kinds(selector: &Selector) -> Option<String> {
    let mut names = Vec::new();
    for pred in &selector.0 {
        match pred {
            Pred::ControlledBy(Ref::Target(0)) => {}
            Pred::Type(kind) => names.push(format!("{kind:?}s").to_lowercase()),
            Pred::AnyOf(any) => {
                for one in any {
                    let Pred::Type(kind) = one else {
                        return None;
                    };
                    names.push(format!("{kind:?}s").to_lowercase());
                }
            }
            _ => return None,
        }
    }
    (!names.is_empty()).then(|| names.join(" and "))
}

impl Writer<'_> {
    /// The selector names permanents of the one target player ("lands target player controls").
    pub(super) fn rmt2_target_players(&self, selector: &Selector) -> bool {
        selector.0.contains(&Pred::ControlledBy(Ref::Target(0)))
            && self.target_is(|what| matches!(what, Targetable::Player | Targetable::Opponent))
            && self.single(&Ref::Target(0))
            && kinds(selector).is_some()
    }

    /// "lands target player controls don't untap during their next untap step".
    pub(super) fn rmt2_held_down_of_target(&mut self, selector: &Selector) -> Option<String> {
        let kinds = kinds(selector)?;
        let player = self.name_target()?;
        Some(format!(
            "{kinds} {player} controls don't untap during their next untap step"
        ))
    }
}

/// "during that player's next untap step" for "during their next untap step" after a target player.
pub fn rmt2_held_down_wordings(texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(" controls don't untap during their next untap step"))
        .map(|text| {
            text.replace(
                "during their next untap step",
                "during that player's next untap step",
            )
        })
        .collect();
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "S gains protection from A or from the color of your choice until end of turn" for the two options of an `Either`
/// written one after the other ("S gains protection from A until end of turn or it gains protection from the color of
/// your choice until end of turn").
pub fn rmt2_protection_wordings(texts: &mut Vec<String>) {
    const LATER: &str = " gains protection from the color of your choice until end of turn";
    const TURN: &str = " until end of turn or ";
    let mut found = Vec::new();
    for text in texts.iter() {
        let Some(at) = text.find(" gains protection from ") else {
            continue;
        };
        let head = &text[..at];
        let rest = &text[at + " gains protection from ".len()..];
        let Some(turn) = rest.find(TURN) else {
            continue;
        };
        let first = &rest[..turn];
        let after = &rest[turn + TURN.len()..];
        if first.contains([',', '.', ';']) {
            continue;
        }
        for subject in ["it", "that creature", "that permanent", "~"] {
            if let Some(tail) = after
                .strip_prefix(subject)
                .and_then(|tail| tail.strip_prefix(LATER))
            {
                found.push(format!(
                    "{head} gains protection from {first} or from the color of your choice until end of turn{tail}"
                ));
            }
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
