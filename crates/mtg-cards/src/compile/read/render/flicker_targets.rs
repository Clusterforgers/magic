//! Targets exiled and returned to the battlefield at once: "Exile target creature you control, then return that card to
//! the battlefield under your control.", "Exile two target artifacts, creatures, and/or lands you control, then return
//! those cards to the battlefield under your control."

use super::{Destination, Effect, Ref, Writer};

impl Writer<'_> {
    /// The sentence of the exile of the targets and the move of them onto the battlefield, and the number of effects it
    /// covers.
    pub(super) fn flicker_targets(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Exile {
                what: what @ Ref::Target(0),
                bind: Some(exiled),
                duration: None,
            },
            Effect::Move {
                what: Ref::Bound(moved),
                to:
                    Destination::Battlefield {
                        tapped: false,
                        under,
                        transformed: false,
                        counters,
                    },
            },
            ..,
        ] = effects
        else {
            return None;
        };
        if exiled != moved || !counters.is_empty() {
            return None;
        }
        let single = self.at_most_one(what);
        let owner = match (under, single) {
            (Some(Ref::You), _) => "your",
            (None, true) => "its owner's",
            (None, false) => "their owners'",
            _ => return None,
        };
        let object = self.object(what)?;
        let card = if single { "that card" } else { "those cards" };
        Some((
            format!("exile {object}, then return {card} to the battlefield under {owner} control"),
            2,
        ))
    }
}
