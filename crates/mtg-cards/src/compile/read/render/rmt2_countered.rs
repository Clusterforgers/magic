//! rmt2: a spell put into its owner's library instead of into the graveyard as it is countered ("Counter target spell. If
//! that spell is countered this way, put it on top of its owner's library instead of into that player's graveyard").

use super::Writer;
use crate::effect::{Effect, Place};
use crate::modify::Duration;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::{ReplaceBy, ReplaceWhen};

impl Writer<'_> {
    /// The shield on the countered spell and the counter, and the number of effects they take.
    pub(super) fn rmt2_countered_into_library(
        &mut self,
        effects: &[Effect],
    ) -> Option<(String, usize)> {
        let [
            Effect::Shield {
                when:
                    ReplaceWhen::Countered {
                        what: Selector(preds),
                    },
                by:
                    ReplaceBy::IntoLibrary {
                        place,
                        reveal: false,
                    },
                duration: Duration::UntilEndOfTurn,
            },
            Effect::Counter {
                what: Ref::Target(0),
                bind: None,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let place = match place {
            Place::Top => "on top of",
            Place::Bottom => "on the bottom of",
            _ => return None,
        };
        if preds.as_slice() != [Pred::Is(Ref::Target(0)), Pred::Spell]
            || !self.single(&Ref::Target(0))
        {
            return None;
        }
        let countered = self.effect(&Effect::Counter {
            what: Ref::Target(0),
            bind: None,
        })?;
        Some((
            format!(
                "{countered}. If that spell is countered this way, put it {place} its owner's library instead of into that player's graveyard"
            ),
            2,
        ))
    }
}
