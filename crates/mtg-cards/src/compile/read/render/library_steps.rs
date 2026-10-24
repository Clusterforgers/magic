//! Steps on the library of a target player: "Look at the top card of target player's library. You may put that card on
//! the bottom of that player's library.", "Exile all cards from target player's library, then that player shuffles
//! their hand into their library."

use super::Writer;
use crate::compile::read::library::{self, TOP};
use crate::effect::{Destination, Effect, Place};
use crate::refs::{Pred, Ref, Targetable, ZoneKind};

impl Writer<'_> {
    /// The target player whose zone a step acts on, named the first time ("target player's") and "that player's" after.
    fn their(&mut self, owner: &Ref) -> Option<String> {
        (*owner == Ref::Target(0)
            && self.single(owner)
            && self.target_is(|what| matches!(what, Targetable::Player | Targetable::Opponent)))
        .then_some(())?;
        if self.named {
            return Some("that player's".to_owned());
        }
        Some(format!("{}'s", self.name_target()?))
    }

    /// Two steps that are one sentence, which no joint splits: a look at the top of a library and the choice to move the
    /// card, or the exile of a library and the shuffle of a hand.
    pub(super) fn library_steps(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        match effects {
            [
                Effect::LookAtTop { who, count, bind },
                Effect::May {
                    who: Ref::You,
                    effects: inner,
                },
                ..,
            ] => {
                let [
                    Effect::Move {
                        what: Ref::Bound(moved),
                        to: Destination::Library { place, order: None },
                    },
                ] = inner.as_slice()
                else {
                    return None;
                };
                (bind == TOP && moved == bind).then_some(())?;
                let whose = self.their(who)?;
                let top = library::top_text(count)?;
                let place = library::place_text(*place)?;
                Some((
                    format!(
                        "look at {top} of {whose} library. You may put that card {place} that player's library"
                    ),
                    2,
                ))
            }
            [
                exile @ Effect::Exile { .. },
                shuffle @ Effect::Move { .. },
                ..,
            ] => {
                let first = self.library_exile(exile)?;
                let second = self.hand_shuffled(shuffle)?;
                Some((format!("{first}, then {second}"), 2))
            }
            _ => None,
        }
    }

    /// An exile of every card in the library of a target player.
    pub(super) fn library_exile(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Exile {
            what: Ref::Each(selector),
            duration: None,
            ..
        } = effect
        else {
            return None;
        };
        let [Pred::In(ZoneKind::Library), Pred::OwnedBy(owner)] = selector.0.as_slice() else {
            return None;
        };
        Some(format!(
            "exile all cards from {} library",
            self.their(owner)?
        ))
    }

    /// A target player shuffles their hand into their library.
    pub(super) fn hand_shuffled(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Move {
            what: Ref::Each(selector),
            to:
                Destination::Library {
                    place: Place::Shuffled,
                    order: None,
                },
        } = effect
        else {
            return None;
        };
        let [Pred::In(ZoneKind::Hand), Pred::OwnedBy(owner)] = selector.0.as_slice() else {
            return None;
        };
        let player = self.their(owner)?.strip_suffix("'s")?.to_owned();
        Some(format!("{player} shuffles their hand into their library"))
    }
}
