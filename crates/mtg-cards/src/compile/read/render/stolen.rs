//! A creature taken for the turn: "Gain control of target creature an opponent controls until end of turn. Untap that
//! creature. It gains haste until end of turn."

use super::Writer;
use crate::compile::read::selectors::keyword_text;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Targetable};
use mtg_core::card::CardType;

impl Writer<'_> {
    /// The control of the one target creature for the turn, its untap and the keyword it gains, as sentences that no joint
    /// splits, and the number of effects they cover.
    pub(super) fn stolen(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Continuous {
                affected: Ref::Target(0),
                mods: take,
                duration: Duration::UntilEndOfTurn,
            },
            Effect::Untap {
                what: Ref::Target(0),
            },
            Effect::Continuous {
                affected: Ref::Target(0),
                mods: gain,
                duration: Duration::UntilEndOfTurn,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let [Modification::AddKeyword(keyword)] = gain.as_slice() else {
            return None;
        };
        // "creature", or "permanent" for a target that names no kind.
        let noun = if self.target_is(|what| {
            matches!(what, Targetable::Object(selector)
                if selector.0.contains(&Pred::Type(CardType::Creature)))
        }) {
            "creature"
        } else if self
            .target_is(|what| matches!(what, Targetable::Object(selector) if selector.0.is_empty()))
        {
            "permanent"
        } else {
            return None;
        };
        let taken = self.control(&Ref::Target(0), take, &Duration::UntilEndOfTurn)?;
        Some((
            format!(
                "{taken}. Untap that {noun}. It gains {} until end of turn",
                keyword_text(keyword)?
            ),
            3,
        ))
    }
}
