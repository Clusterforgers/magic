//! The sentence of a choice of colours and the object that becomes them: "target creature becomes the color or colors
//! of your choice until end of turn".

use super::super::g10d_spell_costs::{g10d_colors_parts, g10d_colors_text};
use super::{Effect, Writer};

impl Writer<'_> {
    /// The choice of colours and the change that opens the effects, as one sentence covering both.
    pub(super) fn g10d_colors(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let [choose, change, ..] = effects else {
            return None;
        };
        let (affected, duration) = g10d_colors_parts(choose, change)?;
        let subject = self.object(affected)?;
        Some((vec![g10d_colors_text(&subject, duration)?], 2))
    }
}
