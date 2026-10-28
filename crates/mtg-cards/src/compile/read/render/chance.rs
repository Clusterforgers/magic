//! The sentences of a coin flip with its branches and of a die roll (the rows of its table are lines of their own).

use super::super::repeat::bound_name;
use super::super::vote::{VOTER, more_votes};
use super::{Effect, Writer};
use crate::effect::VoteFor;
use crate::refs::Ref;
use crate::value::Condition;

impl Writer<'_> {
    /// "flip a coin", "if you win the flip, S" and "if you lose the flip, S" as sentences, or "roll a d20", and the number
    /// of effects they cover.
    pub(super) fn chance(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        match effects.first()? {
            Effect::FlipCoin {
                who,
                win,
                lose,
                lose_first,
                until,
            } => {
                let (opening, subject) = match (who, until) {
                    (Ref::You, false) => ("flip a coin", "you"),
                    (Ref::You, true) => ("flip a coin until you lose a flip", "you"),
                    (Ref::Triggering, false) => ("that player flips a coin", "they"),
                    _ => return None,
                };
                // A flip until a loss is read with no branch.
                if *until && !(win.is_empty() && lose.is_empty()) {
                    return None;
                }
                let mut sentences = vec![opening.to_owned()];
                let neutral = Condition::All(Vec::new());
                let mut branches = [(win, "win"), (lose, "lose")];
                if *lose_first {
                    branches.reverse();
                }
                for (branch, result) in branches {
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
                dice: 1,
                choose: false,
                plus,
                ..
            } if plus.is_zero() => Some((vec![format!("roll a d{sides}")], 1)),
            Effect::RollDice {
                who: Ref::You,
                sides,
                dice: 2,
                choose: true,
                ..
            } => Some((
                vec![format!("roll two d{sides} and choose one result")],
                1,
            )),
            Effect::Vote {
                options: VoteFor::Named(names),
                secret,
                bind: None,
            } => self.vote(names, *secret, &effects[1..]),
            Effect::RepeatEach {
                over,
                bind,
                effects: steps,
            } if bound_name(over) == Some(bind.as_str()) => {
                let each = if *over == Ref::Opponents {
                    "opponent"
                } else {
                    "player"
                };
                let steps = self.consequence(&Condition::All(Vec::new()), steps)?;
                // The subject is said where a bare verb would not say whose tokens they are.
                let you = if steps.starts_with("create ") {
                    "you "
                } else {
                    ""
                };
                Some((vec![format!("for each {each}, {you}{steps}")], 1))
            }
            _ => None,
        }
    }

    /// "starting with you, each player votes for A or B" and what the effects after it do with the votes: "if A gets more
    /// votes, S" and "if B gets more votes or the vote is tied, S", or "for each A vote, S" and "for each B vote, S".
    fn vote(
        &mut self,
        names: &[String],
        secret: bool,
        after: &[Effect],
    ) -> Option<(Vec<String>, usize)> {
        let [first, second] = names else {
            return None;
        };
        let mut sentences = vec![if secret {
            format!(
                "each player secretly votes for {first} or {second}, then those votes are revealed"
            )
        } else {
            format!("starting with you, each player votes for {first} or {second}")
        }];
        let neutral = Condition::All(Vec::new());
        match after {
            [
                Effect::If {
                    condition,
                    then,
                    otherwise,
                },
                ..,
            ] if *condition == more_votes(first, second) => {
                sentences.push(format!(
                    "if {first} gets more votes, {}",
                    self.consequence(&neutral, then)?
                ));
                sentences.push(format!(
                    "if {second} gets more votes or the vote is tied, {}",
                    self.consequence(&neutral, otherwise)?
                ));
                Some((sentences, 2))
            }
            [
                Effect::RepeatEach {
                    over: Ref::Bound(one),
                    bind: voter,
                    effects: of_one,
                },
                Effect::RepeatEach {
                    over: Ref::Bound(other),
                    bind: voter_too,
                    effects: of_other,
                },
                ..,
            ] if one == first && other == second && voter == VOTER && voter_too == VOTER => {
                for (name, steps) in [(first, of_one), (second, of_other)] {
                    sentences.push(format!(
                        "for each {name} vote, {}",
                        self.consequence(&neutral, steps)?
                    ));
                }
                Some((sentences, 3))
            }
            _ => Some((sentences, 1)),
        }
    }
}
