//! A choice about a group: "you may have other attacking creatures get +X/+X until end of turn".

use super::Writer;
use crate::effect::Effect;
use crate::modify::Duration;

impl Writer<'_> {
    /// "you may have target creature get +2/+2 until end of turn": the pump the player may choose to give.
    pub(super) fn may_have(&mut self, effects: &[Effect]) -> Option<String> {
        let [
            Effect::Continuous {
                affected,
                mods,
                duration: Duration::UntilEndOfTurn,
            },
        ] = effects
        else {
            return None;
        };
        let pump = self.pump(affected, mods)?;
        Some(format!(
            "you may have {}",
            pump.replacen(" gets ", " get ", 1)
                .replacen(" gains ", " gain ", 1)
        ))
    }
}
