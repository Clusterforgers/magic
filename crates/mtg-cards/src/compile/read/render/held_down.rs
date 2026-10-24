//! A creature that stays tapped while the source stays: "Tap target creature an opponent controls. That creature doesn't
//! untap during its controller's untap step for as long as you control this creature."

use super::Writer;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;

impl Writer<'_> {
    /// The tap of the one target and the rule that it does not untap while the source stays on the battlefield, as
    /// sentences that no joint splits, and the number of effects they cover.
    pub(super) fn held_down(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            tap @ Effect::Tap {
                what: Ref::Target(0),
            },
            Effect::Continuous {
                affected: Ref::Target(0),
                mods,
                duration: Duration::WhileSourceOnBattlefield,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        if mods.as_slice() != [Modification::DoesntUntap] {
            return None;
        }
        let tapped = self.effect(tap)?;
        Some((
            format!(
                "{tapped}. That creature doesn't untap during its controller's untap step for as long as you control ~"
            ),
            2,
        ))
    }
}
