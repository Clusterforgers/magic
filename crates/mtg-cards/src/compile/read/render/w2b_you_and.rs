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
                ..
            },
            Effect::Draw {
                who, count: again, ..
            },
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

impl Writer<'_> {
    /// Your tokens followed by the same tokens for another player: "you and target opponent each create a Treasure token".
    pub(super) fn you_and_each_create(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            mine @ Effect::CreateToken { who: Ref::You, .. },
            theirs @ Effect::CreateToken { who, .. },
            ..,
        ] = effects
        else {
            return None;
        };
        if *who == Ref::You {
            return None;
        }
        let mut same = theirs.clone();
        if let Effect::CreateToken { who, .. } = &mut same {
            *who = Ref::You;
        }
        if same != *mine {
            return None;
        }
        let other = self.player(who)?;
        let text = super::create::text(mine, self.per)?;
        Some((format!("you and {other} each {text}"), 2))
    }
}
