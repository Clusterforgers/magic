//! Modes: "Choose one —" and its bullet lines are one `Modal` effect. Each bullet is read by the
//! effect templates and keeps the targets it asks for, so a mode's `Target(0)` is its own.

use super::effects;
use super::tokens::Reader;
use crate::effect::{Effect, Mode};
use crate::refs::Bounds;

/// The bullet that opens a mode.
pub const BULLET: char = '\u{2022}';

/// The dash that ends the choice: "Choose one —".
pub const DASH: char = '\u{2014}';

/// "Choose one —" and the bullets after it, as one effect; at least two modes.
pub fn modal(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("choose one")?;
    r.punct(DASH).then_some(())?;
    let mut modes = Vec::new();
    while r.punct(BULLET) {
        modes.push(mode(r)?);
    }
    (modes.len() >= 2).then(|| {
        vec![Effect::Modal {
            choose: Bounds::default(),
            repeat: false,
            modes,
        }]
    })
}

/// The sentences of one bullet, up to the next bullet or the end, with targets of their own and a
/// fresh "it": nothing a mode names is known to the others.
fn mode(r: &mut Reader<'_>) -> Option<Mode> {
    let outer = (std::mem::take(&mut r.targets), r.it.take());
    let mut effects = Vec::new();
    let mut read = true;
    while read && !r.end() && !r.at_punct(BULLET) {
        read = effects::sentence(r, &mut effects);
    }
    let (targets, it) = outer;
    let own = std::mem::replace(&mut r.targets, targets);
    r.it = it;
    // A mode within a mode is not a thing a card does.
    let plain = !effects
        .iter()
        .any(|effect| matches!(effect, Effect::Modal { .. }));
    (read && plain && !effects.is_empty()).then_some(Mode {
        label: None,
        targets: own,
        effects,
    })
}
