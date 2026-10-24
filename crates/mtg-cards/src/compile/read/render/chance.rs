//! The sentences of a coin flip with its branches and of a die roll (the rows of its table are lines of their own).

use super::{Effect, Writer};
use crate::refs::Ref;
use crate::value::Condition;

impl Writer<'_> {
    /// "flip a coin", "if you win the flip, S" and "if you lose the flip, S" as sentences, or "roll a d20", and the number
    /// of effects they cover.
    pub(super) fn chance(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        match effects.first()? {
            Effect::FlipCoin { who, win, lose } => {
                let (opening, subject) = match who {
                    Ref::You => ("flip a coin", "you"),
                    Ref::Triggering => ("that player flips a coin", "they"),
                    _ => return None,
                };
                let mut sentences = vec![opening.to_owned()];
                let neutral = Condition::All(Vec::new());
                for (branch, result) in [(win, "win"), (lose, "lose")] {
                    if !branch.is_empty() {
                        let steps = self.consequence(&neutral, branch)?;
                        sentences.push(format!("if {subject} {result} the flip, {steps}"));
                    }
                }
                Some((sentences, 1))
            }
            Effect::RollDice {
                who: Ref::You,
                sides,
                plus,
                ..
            } if plus.is_zero() => Some((vec![format!("roll a d{sides}")], 1)),
            _ => None,
        }
    }
}
