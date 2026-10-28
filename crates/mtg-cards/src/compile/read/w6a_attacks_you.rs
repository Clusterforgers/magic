//! "can't attack you", "can't attack you or planeswalkers you control" and "can't attack planeswalkers you control":
//! the restriction `Modification::CantAttackYou` of a static ability ("Creatures with flying can't attack you or
//! planeswalkers you control.") or the last part of an Aura's or an Equipment's ("Enchanted creature gets +2/+2, has
//! first strike, and can't attack you or planeswalkers you control."). Read and written back.

use super::tokens::Reader;
use crate::modify::{Attacked, Modification};

/// What follows "can't attack": "you or planeswalkers you control", "planeswalkers you control" or "you".
pub fn w6a_whom(r: &mut Reader<'_>) -> Option<Modification> {
    let attacked = if r.words("you or planeswalkers you control") {
        Attacked::Both
    } else if r.words("planeswalkers you control") {
        Attacked::Planeswalkers
    } else if r.word("you") {
        Attacked::Player
    } else {
        return None;
    };
    Some(Modification::CantAttackYou(attacked))
}

/// "can't attack you or planeswalkers you control", the restriction as a predicate.
pub fn w6a_restriction(r: &mut Reader<'_>) -> Option<Modification> {
    r.attempt(|r| {
        r.words("can't attack").then_some(())?;
        w6a_whom(r)
    })
}

/// The predicate `w6a_restriction` reads, written back.
pub fn w6a_text(modification: &Modification) -> Option<&'static str> {
    match modification {
        Modification::CantAttackYou(Attacked::Both) => {
            Some("can't attack you or planeswalkers you control")
        }
        Modification::CantAttackYou(Attacked::Planeswalkers) => {
            Some("can't attack planeswalkers you control")
        }
        Modification::CantAttackYou(Attacked::Player) => Some("can't attack you"),
        _ => None,
    }
}
