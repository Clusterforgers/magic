//! Auras returned to the battlefield that may enchant only some permanents: "Return each Aura card from your graveyard
//! to the battlefield. Only creatures can be enchanted this way." The second sentence turns the plain move of the first
//! into `Destination::BattlefieldEnchanting`.

use super::selectors::{Form, phrase};
use super::tokens::Reader;
use crate::effect::{Destination, Effect};

/// Reads ". Only <permanents> can be enchanted this way." after a plain move of cards onto the battlefield, and puts the
/// restriction on that move.
pub(super) fn r5515_tail(r: &mut Reader<'_>, effects: &mut [Effect]) -> bool {
    let Some(Effect::Move { to, .. }) = effects.last_mut() else {
        return false;
    };
    let Destination::Battlefield {
        tapped: false,
        under: None,
        transformed: false,
        counters,
    } = to
    else {
        return false;
    };
    if !counters.is_empty() {
        return false;
    }
    let read = r.attempt(|r| {
        r.word("only").then_some(())?;
        let only = phrase(r, Form::All)?;
        r.need("can be enchanted this way")?;
        r.sentence_end().then_some(only)
    });
    let Some(only) = read else {
        return false;
    };
    *to = Destination::BattlefieldEnchanting { only };
    true
}
