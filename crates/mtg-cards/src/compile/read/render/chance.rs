//! The sentences of a coin flip with its branches and of a die roll (the rows of its table are lines of their own).

use super::super::repeat::bound_name;
use super::super::vote::{VOTER, more_votes};
use super::{Effect, Writer};
use crate::effect::{DieRow, VoteFor};
use crate::refs::Ref;
use crate::value::Condition;

/// The sentences of the whole texts (secret numbers, bids and the texts of card pack r37-6) at the head of `effects`,
/// and how many effects they cover.
fn r376_whole_text(effects: &[Effect]) -> Option<(Vec<String>, usize)> {
    super::super::dk27_numbers::dk27_numbers_text(effects)
        .or_else(|| super::super::r376_secret::r376_secret_text(effects))
        .or_else(|| super::super::r376_seats::r376_roll_treasures_text(effects))
        .or_else(|| super::super::dk27_bid::dk27_bid_text(effects))
}

impl Writer<'_> {
    /// "flip five coins. Take an extra turn after this one for each coin that comes up heads": two or more flips of yours
    /// in a row, each with the same `win` list and nothing on a loss, and the number of effects they cover.
    fn ep22_flips(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let first @ Effect::FlipCoin {
            who: Ref::You,
            win,
            lose,
            until: false,
            ..
        } = effects.first()?
        else {
            return None;
        };
        if win.is_empty() || !lose.is_empty() {
            return None;
        }
        let times = effects.iter().take_while(|effect| *effect == first).count();
        if times < 2 {
            return None;
        }
        let count = super::super::amounts::count_word(u32::try_from(times).ok()?)?;
        let steps = self.consequence(&Condition::All(Vec::new()), win)?;
        Some((
            vec![
                format!("flip {count} coins"),
                format!("{steps} for each coin that comes up heads"),
            ],
            times,
        ))
    }

    /// "flip a coin", "if you win the flip, S" and "if you lose the flip, S" as sentences, or "roll a d20", and the number
    /// of effects they cover.
    #[allow(clippy::too_many_lines)]
    pub(super) fn chance(&mut self, effects: &[Effect]) -> Option<(Vec<String>, usize)> {
        let whole = (self.ep22_flips(effects)).or_else(|| r376_whole_text(effects));
        if whole.is_some() {
            return whole;
        }
        match effects.first()? {
            Effect::FlipCoin {
                who,
                win,
                lose,
                lose_first,
                until,
                ..
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
                let mut branches = [(win, "win"), (lose, "lose")];
                if *lose_first {
                    branches.reverse();
                }
                for (branch, result) in branches {
                    if !branch.is_empty() {
                        let steps = self.consequence(&Condition::All(Vec::new()), branch)?;
                        sentences.push(format!("if {subject} {result} the flip, {steps}"));
                    }
                }
                Some((sentences, 1))
            }
            // One row of the lowest results, printed inline after the roll.
            Effect::RollDice {
                who: Ref::You,
                sides,
                dice: 1,
                choose: false,
                plus,
                results,
                count: None,
            } if plus.is_zero()
                && matches!(results.as_slice(), [DieRow { from: 1, to: Some(highest), .. }] if highest < sides) =>
            {
                let [row] = results.as_slice() else {
                    return None;
                };
                let steps = self.consequence(&Condition::All(Vec::new()), &row.effects)?;
                let highest = row.to?;
                Some((
                    vec![
                        format!("roll a d{sides}"),
                        super::super::fix23_dice::fix23_low_row(highest, &steps),
                    ],
                    1,
                ))
            }
            Effect::RollDice {
                who: Ref::You,
                sides,
                dice: 1,
                choose: false,
                plus,
                count: None,
                ..
            } if plus.is_zero() => Some((vec![format!("roll a d{sides}")], 1)),
            Effect::RollDice {
                who: Ref::You,
                sides,
                dice: 2,
                choose: true,
                count: None,
                ..
            } => Some((vec![format!("roll two d{sides} and choose one result")], 1)),
            Effect::Vote {
                options: VoteFor::Named(names),
                secret,
                bind: None,
                tally: None,
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
