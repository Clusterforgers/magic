//! `ChooseOption`: one of a short list of things (a colour, a defending player, a name, the damage a shield takes first).
//!
//! An effect asks through [`pick_options`]: a player with more than one option is asked, the answer is logged as
//! `ChoiceMade` and the step runs again and reads it from the journal (`docs/cards-dsl.md`, Execution).

use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Choice;
use crate::answers::Outcome;
use crate::decision::{Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError};

/// One thing to choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ChoiceOption {
    /// A player.
    Player(PlayerId),
    /// A colour.
    Color(Color),
    /// Anything else, by its name.
    Label(String),
}

/// The things a player may choose between.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OptionOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The things to choose between.
    pub options: Vec<ChoiceOption>,
    /// What is chosen, in the words of the card ("Choose a color."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
}

/// The option chosen, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OptionChoice(pub usize);

/// Checks that the index names one of the options offered.
fn check(options: &[ChoiceOption], index: usize) -> Result<(), IllegalAnswer> {
    if index < options.len() {
        Ok(())
    } else {
        Err(IllegalAnswer::NotOffered("option"))
    }
}

/// Validates the answer. While a cast is being announced the option is the way the spell is cast (`announce`), else it
/// is the answer of an effect or the replacement step and is logged.
pub(crate) fn answer(
    player: PlayerId,
    options: &OptionOptions,
    choice: OptionChoice,
    announcing: bool,
) -> Result<Outcome, EngineError> {
    check(&options.options, choice.0)?;
    if announcing {
        return Ok(Outcome::Way(choice.0));
    }
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::Option(choice.0),
    }]))
}

/// One player who chooses among some options, and what the choice is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OptionChooser {
    /// Who chooses; any seat.
    pub(crate) player: PlayerId,
    /// What they choose between.
    pub(crate) options: Vec<ChoiceOption>,
    /// The question in the words of a card.
    pub(crate) text: String,
}

/// How far the choices of a step have come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Optioned {
    /// Everyone has chosen or had nothing to choose between: the index each took, in the order of the choosers.
    Done(Vec<(PlayerId, usize)>),
    /// The next player who has to choose is asked.
    Ask(Decision),
}

/// Walks the choosers in order and reads the answers already given to this step from its journal.
///
/// A chooser with one option takes it and is not asked, and one with none takes nothing (they are left out of the
/// result). The others are asked one at a time, so a step with k questions runs k + 1 times.
pub(crate) fn pick_options(
    cx: &Cx<'_>,
    choosers: &[OptionChooser],
) -> Result<Optioned, ResolveError> {
    let mut answers = cx.journal().iter();
    let mut taken = Vec::with_capacity(choosers.len());
    for chooser in choosers {
        match chooser.options.len() {
            0 => {}
            1 => taken.push((chooser.player, 0)),
            _ => match answers.next() {
                None => {
                    return Ok(Optioned::Ask(Decision {
                        player: chooser.player,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            source: cx.scope.source.id,
                            options: chooser.options.clone(),
                            text: chooser.text.clone(),
                        }),
                    }));
                }
                Some((player, Choice::Option(index)))
                    if *player == chooser.player && check(&chooser.options, *index).is_ok() =>
                {
                    taken.push((chooser.player, *index));
                }
                Some(other) => {
                    return Err(ResolveError::Invalid(format!(
                        "{other:?} is no answer to the choice of option of {:?}",
                        chooser.player
                    )));
                }
            },
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of option that was not asked".into(),
        ));
    }
    Ok(Optioned::Done(taken))
}

#[cfg(test)]
mod tests {
    //! Goal: an answer to a choice of option is accepted exactly when it names one of the options offered, whatever they are (players, colours, names), and is logged as the chooser's answer; a step that asks it runs once more per question, reads the answers back in order from the journal and takes without asking when there is nothing to choose between, however many players choose.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);
    const THIRD: PlayerId = PlayerId(2);

    fn colors() -> Vec<ChoiceOption> {
        [
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green,
        ]
        .map(ChoiceOption::Color)
        .to_vec()
    }

    fn players() -> Vec<ChoiceOption> {
        vec![ChoiceOption::Player(FOE), ChoiceOption::Player(THIRD)]
    }

    fn labels() -> Vec<ChoiceOption> {
        vec![
            ChoiceOption::Label("Bear".into()),
            ChoiceOption::Label("Elf".into()),
        ]
    }

    fn options(list: Vec<ChoiceOption>) -> OptionOptions {
        OptionOptions {
            source: ObjectId(99),
            options: list,
            text: "Choose one.".into(),
        }
    }

    #[rstest]
    #[case::the_first_colour(colors(), 0)]
    #[case::the_last_colour(colors(), 4)]
    #[case::a_player(players(), 1)]
    #[case::a_name(labels(), 1)]
    fn an_option_that_was_offered_is_logged_as_the_choosers_choice(
        #[case] list: Vec<ChoiceOption>,
        #[case] index: usize,
    ) {
        let outcome = answer(THIRD, &options(list), OptionChoice(index), false).unwrap();
        let Outcome::Events(events) = outcome else {
            panic!("an answer to a choice is logged");
        };
        assert_eq!(
            events,
            [Event::ChoiceMade {
                player: THIRD,
                choice: Choice::Option(index)
            }]
        );
    }

    #[rstest]
    #[case::one_past_the_end(colors(), 5)]
    #[case::far_past_the_end(players(), 99)]
    #[case::an_empty_list(Vec::new(), 0)]
    fn an_index_that_names_no_option_is_refused(
        #[case] list: Vec<ChoiceOption>,
        #[case] index: usize,
    ) {
        assert_eq!(
            answer(ME, &options(list), OptionChoice(index), false).unwrap_err(),
            EngineError::Illegal(IllegalAnswer::NotOffered("option"))
        );
    }

    fn game() -> GameState {
        state(
            vec![vec![], vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![], vec![]],
        )
    }

    fn chooser(player: PlayerId, list: Vec<ChoiceOption>) -> OptionChooser {
        OptionChooser {
            player,
            options: list,
            text: "Choose an opponent.".into(),
        }
    }

    fn pick(
        journal: Vec<(PlayerId, Choice)>,
        choosers: &[OptionChooser],
    ) -> Result<Optioned, ResolveError> {
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
        pick_options(&cx, choosers)
    }

    #[test]
    fn a_chooser_with_options_is_asked_with_the_words_of_the_card() {
        let Optioned::Ask(decision) = pick(vec![], &[chooser(THIRD, players())]).unwrap() else {
            panic!("a question was expected");
        };
        assert_eq!(decision.player, THIRD);
        let DecisionKind::ChooseOption(asked) = decision.kind else {
            panic!("a choice of option was expected");
        };
        assert_eq!(asked.options, players());
        assert_eq!(asked.text, "Choose an opponent.");
    }

    #[rstest]
    #[case::one_option(vec![chooser(ME, vec![ChoiceOption::Player(FOE)])], vec![(ME, 0)])]
    #[case::none(vec![chooser(ME, Vec::new())], vec![])]
    #[case::a_second_chooser_with_none(
        vec![chooser(ME, vec![ChoiceOption::Player(FOE)]), chooser(FOE, Vec::new())],
        vec![(ME, 0)]
    )]
    fn what_there_is_no_choice_about_is_taken_without_a_question(
        #[case] choosers: Vec<OptionChooser>,
        #[case] taken: Vec<(PlayerId, usize)>,
    ) {
        assert_eq!(pick(vec![], &choosers).unwrap(), Optioned::Done(taken));
    }

    #[test]
    fn a_chooser_with_a_choice_is_still_asked_after_one_with_none() {
        let choosers = [
            chooser(ME, vec![ChoiceOption::Player(FOE)]),
            chooser(FOE, colors()),
        ];
        let Optioned::Ask(decision) = pick(vec![], &choosers).unwrap() else {
            panic!("the second chooser is asked");
        };
        assert_eq!(decision.player, FOE);
    }

    #[test]
    fn answers_are_read_in_the_order_of_the_choosers() {
        let choosers = [chooser(FOE, colors()), chooser(THIRD, players())];
        let journal = vec![(FOE, Choice::Option(3)), (THIRD, Choice::Option(1))];
        assert_eq!(
            pick(journal, &choosers).unwrap(),
            Optioned::Done(vec![(FOE, 3), (THIRD, 1)])
        );
        let half = vec![(FOE, Choice::Option(3))];
        let Optioned::Ask(decision) = pick(half, &choosers).unwrap() else {
            panic!("the second chooser is asked");
        };
        assert_eq!(decision.player, THIRD);
    }

    #[rstest]
    #[case::from_another_player(vec![(FOE, Choice::Option(0))])]
    #[case::of_another_kind(vec![(THIRD, Choice::YesNo(true))])]
    #[case::past_the_options(vec![(THIRD, Choice::Option(2))])]
    #[case::to_a_question_not_asked(vec![(THIRD, Choice::Option(0)), (THIRD, Choice::Option(0))])]
    fn a_journal_that_does_not_fit_the_choosers_is_refused(
        #[case] journal: Vec<(PlayerId, Choice)>,
    ) {
        assert!(matches!(
            pick(journal, &[chooser(THIRD, players())]),
            Err(ResolveError::Invalid(_))
        ));
    }
}
