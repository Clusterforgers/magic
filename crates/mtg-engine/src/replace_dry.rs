//! The replacement step run without a player: what a batch of events becomes, for the forecast and the preview.
//!
//! They play events on a scratch copy of the state and send each batch through [`process`] as `Engine::commit` does,
//! so the damage, the life and the deaths they show are the ones the real step produces, in the same order. A question
//! the step asks is answered from [`Picks`]: the first option unless the script says otherwise, which is what the
//! server's clock answers. `Picks` keeps how many options each question had, so a caller can play the others and see
//! whether the numbers depend on the choice.
//!
//! Only the effects of the battlefield are read: one that works from a hand or a library would move a number because of
//! a card nobody may see, and a forecast every seat shares must not.

use mtg_core::ids::PlayerId;

use crate::choose::Choice;
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::replace::{Processed, process};
use crate::state::GameState;

/// The most questions one batch may ask; this bounds a bug, not a game.
const MAX_QUESTIONS: usize = 64;

/// The answers to the questions of the replacement step, in the order they are asked.
#[derive(Debug, Clone, Default)]
pub(crate) struct Picks {
    /// The option taken for the question at each place; 0, the first, past its end.
    script: Vec<usize>,
    /// How many options each question asked so far had.
    asked: Vec<usize>,
}

impl Picks {
    /// Answers that take `script[n]` for the n-th question and the first option after it.
    pub(crate) fn new(script: Vec<usize>) -> Picks {
        Picks {
            script,
            asked: Vec::new(),
        }
    }

    /// How many options each question had, in the order they were asked.
    pub(crate) fn asked(&self) -> &[usize] {
        &self.asked
    }

    fn pick(&mut self, options: usize) -> usize {
        let at = self.asked.len();
        self.asked.push(options);
        self.script
            .get(at)
            .copied()
            .unwrap_or(0)
            .min(options.saturating_sub(1))
    }

    /// The answer to a question of the replacement step.
    fn answer(&mut self, decision: &Decision) -> Result<(PlayerId, Choice), EngineError> {
        let choice = match &decision.kind {
            DecisionKind::ChooseReplacement(options) => {
                Choice::Replacement(self.pick(options.candidates.len()))
            }
            DecisionKind::ChooseOption(options) => Choice::Option(self.pick(options.options.len())),
            DecisionKind::YesNo(options) => {
                Choice::YesNo(self.pick(if options.only_no { 1 } else { 2 }) == 1)
            }
            other => {
                return Err(EngineError::Resolution(format!(
                    "the replacement step asked a {}, which has no default answer",
                    other.name()
                )));
            }
        };
        Ok((decision.player, choice))
    }
}

/// `proposed` after the replacement effects, with the questions of the step answered from `picks`; the state is as it was.
pub(crate) fn settle(
    state: &mut GameState,
    proposed: &[Event],
    picks: &mut Picks,
) -> Result<Vec<Event>, EngineError> {
    let elsewhere = std::mem::replace(&mut state.possible.elsewhere, false);
    let result = rerun(state, proposed, picks);
    state.possible.elsewhere = elsewhere;
    result
}

/// Runs the step again with one more answer each time it asks.
fn rerun(
    state: &GameState,
    proposed: &[Event],
    picks: &mut Picks,
) -> Result<Vec<Event>, EngineError> {
    let mut journal: Vec<(PlayerId, Choice)> = Vec::new();
    loop {
        match process(state, proposed.to_vec(), &journal)? {
            Processed::Events(events) => return Ok(events),
            Processed::Ask { decision, .. } => {
                if journal.len() >= MAX_QUESTIONS {
                    return Err(EngineError::Resolution(
                        "the replacement step went on asking".into(),
                    ));
                }
                journal.push(picks.answer(&decision)?);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the answers of a dry run take the first option unless the script says otherwise, never go past the options of a question, and are recorded in the order of the questions with how many options each had.
    use super::*;
    use crate::choose::{
        ChoiceOption, OptionOptions, ReplacementCandidate, ReplacementOptions, YesNoOptions,
    };
    use mtg_core::ids::ObjectId;
    use rstest::rstest;

    fn replacement(options: usize) -> Decision {
        let candidates = (0..options)
            .map(|at| ReplacementCandidate {
                source: ObjectId(at as u64),
                ability: 0,
                shield: None,
            })
            .collect();
        Decision {
            player: PlayerId(1),
            kind: DecisionKind::ChooseReplacement(ReplacementOptions {
                candidates,
                text: String::new(),
            }),
        }
    }

    #[rstest]
    #[case::no_script(&[], 3, 0)]
    #[case::the_second(&[1], 3, 1)]
    #[case::past_the_options(&[9], 3, 2)]
    #[case::one_option(&[4], 1, 0)]
    fn a_replacement_question_takes_what_the_script_says(
        #[case] script: &[usize],
        #[case] options: usize,
        #[case] taken: usize,
    ) {
        let mut picks = Picks::new(script.to_vec());
        let answer = picks.answer(&replacement(options));
        assert!(matches!(answer, Ok((PlayerId(1), Choice::Replacement(at))) if at == taken));
        assert_eq!(picks.asked(), [options]);
    }

    #[test]
    fn questions_are_answered_in_the_order_they_are_asked() {
        let mut picks = Picks::new(vec![0, 2]);
        let first = picks.answer(&replacement(2));
        let second = picks.answer(&replacement(4));
        assert!(matches!(first, Ok((_, Choice::Replacement(0)))));
        assert!(matches!(second, Ok((_, Choice::Replacement(2)))));
        assert_eq!(picks.asked(), [2, 4]);
    }

    #[rstest]
    #[case::free(false, true)]
    #[case::only_no(true, false)]
    fn a_yes_no_question_is_no_by_default_and_yes_only_when_it_may_be(
        #[case] only_no: bool,
        #[case] yes: bool,
    ) {
        let decision = Decision {
            player: PlayerId(0),
            kind: DecisionKind::YesNo(YesNoOptions {
                only_no,
                ..YesNoOptions::new(ObjectId(1), "Pay 2 life?")
            }),
        };
        assert!(matches!(
            Picks::default().answer(&decision),
            Ok((_, Choice::YesNo(false)))
        ));
        assert!(matches!(
            Picks::new(vec![1]).answer(&decision),
            Ok((_, Choice::YesNo(answer))) if answer == yes
        ));
    }

    #[test]
    fn an_option_question_takes_an_option() {
        let decision = Decision {
            player: PlayerId(0),
            kind: DecisionKind::ChooseOption(OptionOptions {
                source: ObjectId(1),
                options: vec![
                    ChoiceOption::Label("a".into()),
                    ChoiceOption::Label("b".into()),
                ],
                text: String::new(),
            }),
        };
        assert!(matches!(
            Picks::new(vec![1]).answer(&decision),
            Ok((_, Choice::Option(1)))
        ));
    }
}
