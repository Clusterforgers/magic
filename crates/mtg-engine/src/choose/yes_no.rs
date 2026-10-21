//! `YesNo`: "you may", and whether to pay.
//!
//! An effect asks through [`pick_yes_no`]: the player named is asked, the answer is logged as `ChoiceMade` and the
//! step runs again and reads it from the journal (`docs/cards-dsl.md`, Execution).

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Choice;
use crate::answers::Outcome;
use crate::decision::{Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError};

/// A yes-or-no question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct YesNoOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The question in the words of the card ("Pay {1}?"); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
    /// Whether No is the only answer: the payment asked for cannot be made. The question, its words and its clock are
    /// those of a player who could pay, so nothing but this seat's own view tells the two apart.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub only_no: bool,
}

impl YesNoOptions {
    /// A question that either answer is legal to.
    pub fn new(source: ObjectId, text: impl Into<String>) -> Self {
        YesNoOptions {
            source,
            text: text.into(),
            only_no: false,
        }
    }
}

/// The answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct YesNoChoice(pub bool);

/// What an answer does: it is logged. A Yes to a question that only No answers is refused and changes nothing.
pub(crate) fn answer(
    player: PlayerId,
    options: &YesNoOptions,
    choice: YesNoChoice,
) -> Result<Outcome, EngineError> {
    if choice.0 && options.only_no {
        return Err(IllegalAnswer::NotOffered("yes").into());
    }
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::YesNo(choice.0),
    }]))
}

/// How far the questions of a step have come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Said {
    /// Nobody is left to ask: whether any of the players said yes.
    Done(bool),
    /// The next player is asked.
    Ask(Decision),
}

/// Asks the players in order whether they will, until one says yes, and reads the answers already given to this step
/// from its journal. No player at all is a no.
///
/// A step with k questions runs k + 1 times and finds the same players every time.
pub(crate) fn pick_yes_no(
    cx: &Cx<'_>,
    players: &[PlayerId],
    text: &str,
) -> Result<Said, ResolveError> {
    let mut answers = cx.journal().iter();
    let mut said = false;
    for &player in players {
        match answers.next() {
            None => {
                return Ok(Said::Ask(Decision {
                    player,
                    kind: DecisionKind::YesNo(YesNoOptions::new(cx.scope.source.id, text)),
                }));
            }
            Some((from, Choice::YesNo(yes))) if *from == player => {
                if *yes {
                    said = true;
                    break;
                }
            }
            Some(other) => {
                return Err(ResolveError::Invalid(format!(
                    "{other:?} is no answer to the question put to {player:?}"
                )));
            }
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a question that was not asked".into(),
        ));
    }
    Ok(Said::Done(said))
}

#[cfg(test)]
mod tests {
    //! Goal: a yes-or-no answer is logged as the answerer's choice whichever way it goes, and a step that asks it puts the question to each player in order until one says yes, reads the answers back from the journal and refuses an answer from the wrong player, of the wrong kind or to a question that was not asked.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);
    const THIRD: PlayerId = PlayerId(2);

    fn game() -> GameState {
        state(
            vec![vec![], vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![], vec![]],
        )
    }

    fn pick(journal: Vec<(PlayerId, Choice)>, players: &[PlayerId]) -> Result<Said, ResolveError> {
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
        pick_yes_no(&cx, players, "Draw a card?")
    }

    fn asked(said: Said) -> (PlayerId, String) {
        let Said::Ask(Decision {
            player,
            kind: DecisionKind::YesNo(options),
        }) = said
        else {
            panic!("a question was expected, got {said:?}");
        };
        (player, options.text)
    }

    #[rstest]
    #[case::yes(true)]
    #[case::no(false)]
    fn an_answer_is_logged_as_the_answerers_choice(#[case] yes: bool) {
        let options = YesNoOptions::new(ObjectId(1), "");
        let Ok(Outcome::Events(events)) = answer(FOE, &options, YesNoChoice(yes)) else {
            panic!("an answer to a question is logged");
        };
        assert_eq!(
            events,
            [Event::ChoiceMade {
                player: FOE,
                choice: Choice::YesNo(yes)
            }]
        );
    }

    #[rstest]
    #[case::yes_is_refused(true, false)]
    #[case::no_is_the_only_answer(false, true)]
    fn a_question_that_only_no_answers_refuses_a_yes(#[case] yes: bool, #[case] accepted: bool) {
        let options = YesNoOptions {
            only_no: true,
            ..YesNoOptions::new(ObjectId(1), "Pay {2}?")
        };
        let result = answer(FOE, &options, YesNoChoice(yes));
        assert_eq!(result.is_ok(), accepted, "{result:?}");
        if !accepted {
            assert!(matches!(
                result,
                Err(EngineError::Illegal(IllegalAnswer::NotOffered("yes")))
            ));
        }
    }

    #[test]
    fn the_first_player_is_asked_with_the_words_of_the_card() {
        assert_eq!(
            asked(pick(vec![], &[FOE, THIRD]).unwrap()),
            (FOE, "Draw a card?".to_owned())
        );
    }

    #[rstest]
    #[case::the_only_player_says_yes(&[ME], &[(ME, true)], true)]
    #[case::the_only_player_says_no(&[ME], &[(ME, false)], false)]
    #[case::the_first_yes_ends_the_asking(&[FOE, THIRD], &[(FOE, true)], true)]
    #[case::a_no_asks_the_next_player(&[FOE, THIRD], &[(FOE, false), (THIRD, true)], true)]
    #[case::everyone_says_no(&[FOE, THIRD], &[(FOE, false), (THIRD, false)], false)]
    #[case::nobody_to_ask(&[], &[], false)]
    fn the_answers_in_the_journal_decide(
        #[case] players: &[PlayerId],
        #[case] given: &[(PlayerId, bool)],
        #[case] expected: bool,
    ) {
        let journal = given
            .iter()
            .map(|(player, yes)| (*player, Choice::YesNo(*yes)))
            .collect();
        assert_eq!(pick(journal, players).unwrap(), Said::Done(expected));
    }

    #[test]
    fn a_no_puts_the_question_to_the_next_player() {
        let journal = vec![(FOE, Choice::YesNo(false))];
        assert_eq!(
            asked(pick(journal, &[FOE, THIRD]).unwrap()).0,
            THIRD,
            "the question goes on to the next seat"
        );
    }

    #[rstest]
    #[case::from_another_player(vec![(THIRD, Choice::YesNo(true))])]
    #[case::of_another_kind(vec![(FOE, Choice::Option(0))])]
    #[case::to_a_question_not_asked(vec![(FOE, Choice::YesNo(true)), (FOE, Choice::YesNo(true))])]
    fn a_journal_that_does_not_fit_the_questions_is_refused(
        #[case] journal: Vec<(PlayerId, Choice)>,
    ) {
        assert!(matches!(
            pick(journal, &[FOE]),
            Err(ResolveError::Invalid(_))
        ));
    }
}
