//! The sentence of a choice between two sentences: "A or B", each option written as a sentence of its own.

use super::{Condition, Effect, Writer, chosen_move_text};

impl Writer<'_> {
    /// An option of the choice: a card chosen and moved is one sentence, any other list of steps is joined as usual.
    fn dk1c5_option(&mut self, steps: &[Effect]) -> Option<String> {
        match steps {
            [choose @ Effect::Choose { .. }, moved @ Effect::Move { .. }] => {
                chosen_move_text(choose, moved)
            }
            _ => self.consequence(&Condition::All(Vec::new()), steps),
        }
    }

    /// "A or B" for the two options of an `Either`.
    pub(super) fn dk1c5_either_text(
        &mut self,
        first: &[Effect],
        second: &[Effect],
    ) -> Option<String> {
        Some(format!(
            "{} or {}",
            self.dk1c5_option(first)?,
            self.dk1c5_option(second)?
        ))
    }
}
