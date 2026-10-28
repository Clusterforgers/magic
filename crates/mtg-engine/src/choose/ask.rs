//! Asking several questions in one step: the answers given so far sit in the journal in the order they were asked.
//!
//! A step walks its questions in the same order every time it runs. [`Questions`] hands out the next answer from the
//! journal, or stops the step with the decision to put when the journal has none left, so a step with k questions
//! runs k + 1 times and finds the same candidates each time (`docs/cards-dsl.md`, Execution).

use mtg_core::ids::{ObjectId, PlayerId};

use super::order::{self, OrderOptions};
use super::{CardsOptions, Choice, ChoiceOption, OptionOptions, YesNoOptions, cards};
use crate::decision::{Decision, DecisionKind};
use crate::resolve::{Cx, ResolveError, Run};

/// Why a step stops before it is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Stop {
    /// A player has to answer first.
    Ask(Box<Decision>),
    /// The journal or the data does not fit the step.
    Fail(ResolveError),
}

impl From<ResolveError> for Stop {
    fn from(error: ResolveError) -> Self {
        Stop::Fail(error)
    }
}

impl Stop {
    /// The step's result: what it ran, the decision it needs answered, or the error.
    pub(crate) fn settle(result: Result<Run, Stop>) -> Result<Run, ResolveError> {
        match result {
            Ok(run) => Ok(run),
            Err(Stop::Ask(decision)) => Ok(Run::Ask(*decision)),
            Err(Stop::Fail(error)) => Err(error),
        }
    }
}

/// The answers a step has been given, taken in the order the step asks its questions.
pub(crate) struct Questions<'a> {
    source: ObjectId,
    answers: std::slice::Iter<'a, (PlayerId, Choice)>,
}

impl<'a> Questions<'a> {
    /// The questions of the step that runs in `cx`.
    pub(crate) fn new(cx: &'a Cx<'_>) -> Self {
        Questions {
            source: cx.scope.source.id,
            answers: cx.journal().iter(),
        }
    }

    fn unfit(answer: &(PlayerId, Choice), player: PlayerId, what: &str) -> Stop {
        Stop::Fail(ResolveError::Invalid(format!(
            "{answer:?} is no answer to the {what} of {player:?}"
        )))
    }

    /// The cards `player` takes from `from`, between `min` and `max` of them.
    pub(crate) fn cards(
        &mut self,
        player: PlayerId,
        from: Vec<ObjectId>,
        (min, max): (usize, usize),
        text: String,
    ) -> Result<Vec<ObjectId>, Stop> {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let options = CardsOptions {
            source: self.source,
            from,
            min: count(min),
            max: count(max),
            text,
            can_cancel: false,
        };
        match self.answers.next() {
            None => Err(Stop::Ask(Box::new(Decision {
                player,
                kind: DecisionKind::ChooseCards(options),
            }))),
            Some((who, Choice::Cards(chosen)))
                if *who == player && cards::check(&options, chosen).is_ok() =>
            {
                Ok(chosen.clone())
            }
            Some(other) => Err(Self::unfit(other, player, "choice of cards")),
        }
    }

    /// The order `player` puts `cards` in, the card nearest the top first.
    pub(crate) fn order(
        &mut self,
        player: PlayerId,
        cards: Vec<ObjectId>,
        text: String,
    ) -> Result<Vec<ObjectId>, Stop> {
        let options = OrderOptions {
            source: self.source,
            cards,
            text,
        };
        match self.answers.next() {
            None => Err(Stop::Ask(Box::new(Decision {
                player,
                kind: DecisionKind::OrderCards(options),
            }))),
            Some((who, Choice::Order(chosen)))
                if *who == player && order::check(&options, chosen).is_ok() =>
            {
                Ok(chosen.clone())
            }
            Some(other) => Err(Self::unfit(other, player, "order of cards")),
        }
    }

    /// Whether `player` says yes to the question.
    pub(crate) fn yes_no(&mut self, player: PlayerId, text: String) -> Result<bool, Stop> {
        match self.answers.next() {
            None => Err(Stop::Ask(Box::new(Decision {
                player,
                kind: DecisionKind::YesNo(YesNoOptions::new(self.source, text)),
            }))),
            Some((who, Choice::YesNo(yes))) if *who == player => Ok(*yes),
            Some(other) => Err(Self::unfit(other, player, "yes-or-no question")),
        }
    }

    /// The index of the option `player` chooses; one option is taken without asking.
    pub(crate) fn w9b_option(
        &mut self,
        player: PlayerId,
        options: Vec<ChoiceOption>,
        text: String,
    ) -> Result<usize, Stop> {
        if options.len() < 2 {
            return Ok(0);
        }
        match self.answers.next() {
            None => Err(Stop::Ask(Box::new(Decision {
                player,
                kind: DecisionKind::ChooseOption(OptionOptions {
                    source: self.source,
                    options,
                    text,
                }),
            }))),
            Some((who, Choice::Option(index))) if *who == player && *index < options.len() => {
                Ok(*index)
            }
            Some(other) => Err(Self::unfit(other, player, "choice of option")),
        }
    }

    /// Ends the walk: an answer nobody asked for is a bug.
    pub(crate) fn end(mut self) -> Result<(), ResolveError> {
        if self.answers.next().is_some() {
            return Err(ResolveError::Invalid(
                "an answer was given to a question that was not asked".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a step that asks several questions finds each answer in the journal in the order it asks, puts the next question when the journal has run out, refuses an answer of the wrong player, kind or content as a bug and refuses an answer left over.
    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn ids(numbers: &[u64]) -> Vec<ObjectId> {
        numbers.iter().copied().map(ObjectId).collect()
    }

    /// Runs `walk` for the source on the battlefield with `journal` as the answers given so far.
    fn walk<T>(
        journal: Vec<(PlayerId, Choice)>,
        walk: impl FnOnce(&mut Questions<'_>) -> Result<T, Stop>,
    ) -> Result<T, Stop> {
        let game = game();
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let mut questions = Questions::new(&cx);
        let result = walk(&mut questions)?;
        questions.end()?;
        Ok(result)
    }

    fn both(questions: &mut Questions<'_>) -> Result<(Vec<ObjectId>, Vec<ObjectId>), Stop> {
        let bottom = questions.cards(ME, ids(&[1, 2, 3]), (0, 3), "Bottom.".into())?;
        let top = questions.order(FOE, ids(&[4, 5]), "Order.".into())?;
        Ok((bottom, top))
    }

    #[test]
    fn the_first_question_without_an_answer_is_put_with_the_source_and_the_words() {
        let Err(Stop::Ask(decision)) = walk(Vec::new(), both) else {
            panic!("the first question is put");
        };
        let source = game().zone(Zone::Battlefield)[0];
        assert_eq!(
            *decision,
            Decision {
                player: ME,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source,
                    from: ids(&[1, 2, 3]),
                    min: 0,
                    max: 3,
                    text: "Bottom.".into(),
                    can_cancel: false,
                })
            }
        );
    }

    #[test]
    fn the_second_question_is_put_once_the_first_is_answered_and_both_are_read_back_in_order() {
        let first = (ME, Choice::Cards(ids(&[2])));
        let Err(Stop::Ask(decision)) = walk(vec![first.clone()], both) else {
            panic!("the second question is put");
        };
        assert_eq!(decision.player, FOE);
        assert!(
            matches!(decision.kind, DecisionKind::OrderCards(ref o) if o.cards == ids(&[4, 5]))
        );
        let second = (FOE, Choice::Order(ids(&[5, 4])));
        let done = walk(vec![first, second], both).unwrap();
        assert_eq!(done, (ids(&[2]), ids(&[5, 4])));
    }

    #[test]
    fn an_answer_that_does_not_fit_is_a_bug_named_as_one() {
        let good = (ME, Choice::Cards(ids(&[1])));
        let wrong_player = (FOE, Choice::Cards(ids(&[1])));
        let not_offered = (ME, Choice::Cards(ids(&[9])));
        let wrong_kind = (ME, Choice::Order(ids(&[1])));
        for first in [wrong_player, not_offered, wrong_kind] {
            let refused = walk(vec![first.clone()], both);
            assert!(
                matches!(refused, Err(Stop::Fail(ResolveError::Invalid(_)))),
                "{first:?}: {refused:?}"
            );
        }
        let bad_order = (FOE, Choice::Order(ids(&[4])));
        let refused = walk(vec![good, bad_order], both);
        assert!(matches!(refused, Err(Stop::Fail(ResolveError::Invalid(_)))));
    }

    #[test]
    fn an_answer_nobody_asked_for_is_refused() {
        let journal = vec![
            (ME, Choice::Cards(ids(&[1]))),
            (FOE, Choice::Order(ids(&[4, 5]))),
            (ME, Choice::YesNo(true)),
        ];
        let refused = walk(journal, both);
        assert!(matches!(refused, Err(Stop::Fail(ResolveError::Invalid(_)))));
    }

    #[test]
    fn a_stop_settles_into_the_runs_of_a_step() {
        let decision = Decision {
            player: ME,
            kind: DecisionKind::OrderCards(OrderOptions {
                source: ObjectId(1),
                cards: ids(&[2, 3]),
                text: String::new(),
            }),
        };
        assert_eq!(
            Stop::settle(Err(Stop::Ask(Box::new(decision.clone())))).unwrap(),
            Run::Ask(decision)
        );
        assert_eq!(
            Stop::settle(Ok(Run::events(Vec::new()))).unwrap(),
            Run::events(Vec::new())
        );
        let error = ResolveError::Invalid("x".into());
        assert_eq!(Stop::settle(Err(error.clone().into())).unwrap_err(), error);
    }
}
