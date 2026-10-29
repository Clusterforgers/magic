//! The sentences of the clash wordings of `w11c_clash`: the clash, then the return, whose win may put the creature on
//! top instead, and the clash whose win holds the clashed player's creatures through their next untap step.

use super::super::w9b_clash::W9B_CLASH;
use super::super::w11c_clash::{W11C_HELD, W11C_INSTEAD, w11c_held, w11c_on_top, w11c_to_hand};
use super::{Effect, Writer};
use crate::refs::Ref;
use crate::value::Condition;

impl Writer<'_> {
    /// The clash wordings of `w11c_clash` that open the effects, as sentences, and the number of effects they cover.
    pub(super) fn w11c_clash(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let Some(Effect::Clash {
            who: Ref::You,
            win,
            lose,
            until: false,
        }) = effects.first()
        else {
            return None;
        };
        lose.is_empty().then_some(())?;
        if *win == [w11c_held()] {
            return Some((
                vec![W9B_CLASH.to_owned(), format!("if you win, {W11C_HELD}")],
                1,
            ));
        }
        (*win == [w11c_on_top()] && effects.get(1) == Some(&w11c_to_hand())).then_some(())?;
        let moved = self.consequence(&Condition::All(Vec::new()), &effects[1..2])?;
        Some((
            vec![
                format!("{W9B_CLASH}, then {moved}"),
                format!("if you win, {W11C_INSTEAD}"),
            ],
            2,
        ))
    }
}
