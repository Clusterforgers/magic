//! "you and target opponent each draw two cards": your draw and another player's of the same count, as one sentence.

use super::super::amounts::cards_text;
use super::{Effect, Writer};
use crate::refs::Ref;

impl Writer<'_> {
    /// Your draw followed by another player's draw of as many cards, and the two effects it covers.
    pub(super) fn w2b_you_and_each(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Draw {
                who: Ref::You,
                count,
            },
            Effect::Draw { who, count: again },
            ..,
        ] = effects
        else {
            return None;
        };
        if count != again || *who == Ref::You {
            return None;
        }
        let cards = cards_text(count)?;
        let other = self.player(who)?;
        Some((format!("you and {other} each draw {cards}"), 2))
    }
}
