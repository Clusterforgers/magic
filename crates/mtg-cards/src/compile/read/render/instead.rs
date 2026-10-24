//! A move the card says another way when a land entered is a basic land type: "you may return target nonland permanent
//! card from your graveyard to your hand. If that land is a Plains, you may return that nonland permanent card to the
//! battlefield instead."

use super::{Destination, Effect, Writer, spec_text};
use crate::refs::{Pred, Ref};
use crate::value::Condition;

/// The basic land types a land that entered can be.
const LAND_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// The one card a `May` moves to `to`: "you may return target ... to your hand".
fn may_move(effects: &[Effect], to: &Destination) -> bool {
    matches!(
        effects,
        [Effect::May { who: Ref::You, effects }]
            if matches!(effects.as_slice(), [Effect::Move { what: Ref::Target(0), to: found }] if found == to)
    )
}

impl Writer<'_> {
    /// The sentence of a move to the hand that a basic land type changes to a move to the battlefield, and the
    /// number of effects they cover.
    pub(super) fn move_instead(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::If {
                condition:
                    Condition::Matches {
                        subject: Ref::Triggering,
                        selector,
                    },
                then,
                otherwise,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let [Pred::Subtype(land)] = selector.0.as_slice() else {
            return None;
        };
        let battlefield = Destination::Battlefield {
            tapped: false,
            under: None,
            transformed: false,
            counters: Vec::new(),
        };
        if !LAND_TYPES.contains(&land.0.as_str())
            || !may_move(then, &battlefield)
            || !may_move(otherwise, &Destination::Hand)
        {
            return None;
        }
        let kind = spec_text(self.targets.first()?)?;
        let kind = kind
            .strip_prefix("target ")?
            .strip_suffix(" from your graveyard")?
            .to_owned();
        let first = self.effect(otherwise.first()?)?;
        Some((
            format!(
                "{first}. If that land is a {}, you may return that {kind} to the battlefield instead",
                land.0
            ),
            1,
        ))
    }
}
