//! The sentences of the clash wordings of `w15c_clash`: a clash with the defending player, and a won clash whose delayed
//! trigger may add colorless mana at the beginning of the next main phase.

use super::super::w9b_clash::W9B_CLASH;
use super::super::w15c_clash::{
    W15C_DEFENDING, W15C_NEXT_MAIN, w15c_clash_effect, w15c_next_main_mana,
};
use super::{Effect, Writer};
use crate::refs::Ref;

impl Writer<'_> {
    /// The clash wordings of `w15c_clash` that open the effects, as sentences, and the number of effects they cover.
    pub(super) fn w15c_clash(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let first = effects.first()?;
        if *first == w15c_clash_effect(Vec::new(), Some(Ref::Defending)) {
            return Some((vec![W15C_DEFENDING.to_owned()], 1));
        }
        (*first == w15c_clash_effect(vec![w15c_next_main_mana()], None)).then_some(())?;
        Some((
            vec![
                W9B_CLASH.to_owned(),
                format!(
                    "if you win, {W15C_NEXT_MAIN}, you may add an amount of {{C}} equal to that spell's mana value"
                ),
            ],
            1,
        ))
    }
}
