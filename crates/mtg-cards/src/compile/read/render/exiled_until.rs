//! An opponent who exiles cards from the top of their library until one is found, the free cast of it and the move of the
//! rest to the bottom: "Target opponent exiles cards from the top of their library until they exile an instant or sorcery
//! card. You may cast that card without paying its mana cost. Then put the exiled cards that weren't cast this way on the
//! bottom of that library in a random order."

use super::{Destination, Effect, Order, Place, Ref, Targetable, Writer};
use crate::effect::{EXILED, FOUND};

impl Writer<'_> {
    /// The three sentences when the player who exiles is the one target, an opponent; the number of effects they take.
    pub(super) fn opponent_exiles(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::ExileUntil {
                who: Ref::Target(0),
                filter,
                bind,
                exiled_as,
            },
            cast,
            moved,
            ..,
        ] = effects
        else {
            return None;
        };
        let found = Ref::Bound(FOUND.to_owned());
        let cast_it = Effect::CastFromZone {
            who: Ref::You,
            what: found,
            free: true,
            play: false,
            way: None,
            transformed: false,
            bind: None,
            otherwise: Vec::new(),
        };
        let to_the_bottom = Effect::Move {
            what: Ref::Bound(EXILED.to_owned()),
            to: Destination::Library {
                place: Place::Bottom,
                order: Some(Order::Random),
            },
        };
        (bind == FOUND && exiled_as == EXILED && *cast == cast_it && *moved == to_the_bottom)
            .then_some(())?;
        (self.single(&Ref::Target(0))
            && self.target_is(|what| matches!(what, Targetable::Opponent)))
        .then_some(())?;
        let kind = super::library::filter_text(filter, false)?;
        let target = self.name_target()?;
        Some((
            format!(
                "{target} exiles cards from the top of their library until they exile {} {kind}. You may cast that card without paying its mana cost. Then put the exiled cards that weren't cast this way on the bottom of that library in a random order",
                super::article(&kind)
            ),
            3,
        ))
    }
}
