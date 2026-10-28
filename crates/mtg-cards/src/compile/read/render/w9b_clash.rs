//! The sentences of a clash: "clash with an opponent" and "if you win, S", or the steps before a clash that repeats them
//! with ", then" and "if you win, repeat this process".

use super::super::w9b_clash::{W9B_CLASH, W9B_REPEAT};
use super::{Effect, Writer};
use crate::refs::Ref;
use crate::value::{Amount, Condition};

impl Writer<'_> {
    /// The clash that opens the effects, or the steps of a clash until a loss and the clash, as sentences, and the number
    /// of effects they cover.
    pub(super) fn w9b_clash(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let neutral = Condition::All(Vec::new());
        if let Some(at) = effects
            .iter()
            .position(|effect| matches!(effect, Effect::Clash { until: true, .. }))
            && at > 0
        {
            let Effect::Clash {
                who: Ref::You,
                win,
                lose,
                ..
            } = &effects[at]
            else {
                return None;
            };
            (win.as_slice() == &effects[..at] && lose.is_empty()).then_some(())?;
            let steps = self.consequence(&neutral, win)?;
            let steps = if steps.starts_with("you ") {
                steps
            } else {
                format!("you {steps}")
            };
            return Some((
                vec![
                    format!("{steps}, then {W9B_CLASH}"),
                    format!("if you win, {W9B_REPEAT}"),
                ],
                at + 1,
            ));
        }
        let Effect::Clash {
            who: Ref::You,
            win,
            lose,
            until: false,
        } = effects.first()?
        else {
            return None;
        };
        lose.is_empty().then_some(())?;
        let mut sentences = vec![W9B_CLASH.to_owned()];
        if !win.is_empty() {
            let steps = match win.as_slice() {
                [
                    Effect::DealDamage {
                        source: None,
                        to: Ref::ControllerOf(target),
                        amount: Amount::Fixed(amount),
                    },
                ] if **target == Ref::Target(0) => {
                    format!("~ deals {amount} damage to that creature's controller")
                }
                _ => self.consequence(&neutral, win)?,
            };
            sentences.push(format!("if you win, {steps}"));
        }
        Some((sentences, 1))
    }
}
