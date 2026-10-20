//! `ChooseCards`: a player picks cards from a list (discard, sacrifice; search and reveal follow).
//!
//! An effect asks through [`pick_cards`]: a player with no more cards to choose from than they must take takes them
//! all and is not asked; any other is asked, the answer is logged as `ChoiceMade` and the step runs again and reads
//! it from the journal (`docs/cards-dsl.md`, Execution).

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use super::Choice;
use crate::answers::Outcome;
use crate::decision::{Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError};

/// The cards a player may choose from and how many.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardsOptions {
    /// The object whose effect asks.
    pub source: ObjectId,
    /// The cards that may be chosen.
    pub from: Vec<ObjectId>,
    /// The fewest to choose.
    pub min: u32,
    /// The most to choose.
    pub max: u32,
    /// What the choice is for, in the words of a card ("Discard a card."); empty when the engine gave none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<String>", optional))]
    pub text: String,
}

/// The cards chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardsChoice(pub Vec<ObjectId>);

/// Checks that `cards` are all among the cards offered, each once, and between `min` and `max` of them.
pub(super) fn check(options: &CardsOptions, cards: &[ObjectId]) -> Result<(), IllegalAnswer> {
    for (at, card) in cards.iter().enumerate() {
        if cards[..at].contains(card) {
            return Err(IllegalAnswer::Duplicate("card"));
        }
        if !options.from.contains(card) {
            return Err(IllegalAnswer::NotOffered("card"));
        }
    }
    let count = u32::try_from(cards.len()).unwrap_or(u32::MAX);
    let expected = if count < options.min {
        options.min
    } else if count > options.max {
        options.max
    } else {
        return Ok(());
    };
    Err(IllegalAnswer::WrongCount {
        expected: usize::try_from(expected).unwrap_or(usize::MAX),
        got: cards.len(),
    })
}

pub(crate) fn answer(
    player: PlayerId,
    options: &CardsOptions,
    choice: CardsChoice,
) -> Result<Outcome, EngineError> {
    check(options, &choice.0)?;
    Ok(Outcome::Events(vec![Event::ChoiceMade {
        player,
        choice: Choice::Cards(choice.0),
    }]))
}

/// One player who takes cards from a list, and what the choice is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chooser {
    /// Who chooses; any seat.
    pub(crate) player: PlayerId,
    /// The cards they may take.
    pub(crate) from: Vec<ObjectId>,
    /// How many they take.
    pub(crate) take: usize,
    /// The question in the words of a card.
    pub(crate) text: String,
}

/// How far the choices of a step have come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Picked {
    /// Everyone has chosen or had nothing to choose: the cards each takes, in the order of the choosers.
    Done(Vec<(PlayerId, Vec<ObjectId>)>),
    /// The next player who has to choose is asked.
    Ask(Decision),
}

/// Walks the choosers in order and reads the answers already given to this step from its journal.
///
/// A chooser who takes nothing, or has no more cards than they take, takes them all and is not asked. The others
/// are asked one at a time, so a step with k questions runs k + 1 times and finds the same candidates every time.
pub(crate) fn pick_cards(cx: &Cx<'_>, choosers: &[Chooser]) -> Result<Picked, ResolveError> {
    let mut answers = cx.journal().iter();
    let mut taken = Vec::with_capacity(choosers.len());
    for chooser in choosers {
        if chooser.take == 0 {
            taken.push((chooser.player, Vec::new()));
        } else if chooser.from.len() <= chooser.take {
            taken.push((chooser.player, chooser.from.clone()));
        } else {
            let take = u32::try_from(chooser.take).unwrap_or(u32::MAX);
            let options = CardsOptions {
                source: cx.scope.source.id,
                from: chooser.from.clone(),
                min: take,
                max: take,
                text: chooser.text.clone(),
            };
            match answers.next() {
                None => {
                    return Ok(Picked::Ask(Decision {
                        player: chooser.player,
                        kind: DecisionKind::ChooseCards(options),
                    }));
                }
                Some((player, Choice::Cards(cards)))
                    if *player == chooser.player && check(&options, cards).is_ok() =>
                {
                    taken.push((chooser.player, cards.clone()));
                }
                Some(other) => {
                    return Err(ResolveError::Invalid(format!(
                        "{other:?} is no answer to the choice of cards of {:?}",
                        chooser.player
                    )));
                }
            }
        }
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of cards that was not asked".into(),
        ));
    }
    Ok(Picked::Done(taken))
}

#[cfg(test)]
mod tests {
    //! Goal: an answer to a choice of cards is accepted exactly when it names cards among those offered, each once, between the fewest and the most, and is logged as the chooser's answer; a step that asks it runs once more per question, reads the answers back in order from the journal and takes without asking when there is nothing to choose, however many players choose.
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn ids(numbers: &[u64]) -> Vec<ObjectId> {
        numbers.iter().copied().map(ObjectId).collect()
    }

    fn options(from: &[u64], min: u32, max: u32) -> CardsOptions {
        CardsOptions {
            source: ObjectId(99),
            from: ids(from),
            min,
            max,
            text: "Discard a card.".into(),
        }
    }

    #[rstest]
    #[case::exactly_one_of_three(&[1, 2, 3], 1, 1, &[2])]
    #[case::the_most_of_a_range(&[1, 2, 3], 1, 2, &[3, 1])]
    #[case::the_fewest_of_a_range(&[1, 2, 3], 1, 2, &[1])]
    #[case::none_when_none_are_needed(&[1, 2, 3], 0, 2, &[])]
    #[case::every_card(&[1, 2], 2, 2, &[2, 1])]
    fn a_legal_answer_is_logged_as_the_choosers_choice(
        #[case] from: &[u64],
        #[case] min: u32,
        #[case] max: u32,
        #[case] chosen: &[u64],
    ) {
        let outcome = answer(FOE, &options(from, min, max), CardsChoice(ids(chosen))).unwrap();
        let Outcome::Events(events) = outcome else {
            panic!("an answer to a choice is logged");
        };
        assert_eq!(
            events,
            [Event::ChoiceMade {
                player: FOE,
                choice: Choice::Cards(ids(chosen))
            }]
        );
    }

    #[rstest]
    #[case::a_card_that_was_not_offered(&[1, 2, 3], 1, 1, &[4], IllegalAnswer::NotOffered("card"))]
    #[case::one_offered_and_one_not(&[1, 2, 3], 2, 2, &[1, 9], IllegalAnswer::NotOffered("card"))]
    #[case::the_same_card_twice(&[1, 2, 3], 2, 2, &[2, 2], IllegalAnswer::Duplicate("card"))]
    #[case::a_duplicate_before_the_count_is_judged(&[1, 2, 3], 1, 1, &[2, 2], IllegalAnswer::Duplicate("card"))]
    #[case::too_few(&[1, 2, 3], 2, 3, &[1], IllegalAnswer::WrongCount { expected: 2, got: 1 })]
    #[case::nothing_when_something_is_needed(&[1, 2, 3], 1, 1, &[], IllegalAnswer::WrongCount { expected: 1, got: 0 })]
    #[case::too_many(&[1, 2, 3], 1, 2, &[1, 2, 3], IllegalAnswer::WrongCount { expected: 2, got: 3 })]
    fn an_illegal_answer_is_refused_by_name(
        #[case] from: &[u64],
        #[case] min: u32,
        #[case] max: u32,
        #[case] chosen: &[u64],
        #[case] refusal: IllegalAnswer,
    ) {
        let error = answer(FOE, &options(from, min, max), CardsChoice(ids(chosen))).unwrap_err();
        assert!(
            matches!(&error, EngineError::Illegal(got) if *got == refusal),
            "{error:?}"
        );
    }

    /// Seat 0 has a hand of three cards, seat 1 a hand of one.
    fn game() -> GameState {
        state(
            vec![
                vec![
                    creature("A", 1, 1),
                    creature("B", 1, 1),
                    creature("C", 1, 1),
                ],
                vec![creature("D", 1, 1)],
            ],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn chooser(game: &GameState, player: PlayerId, take: usize) -> Chooser {
        Chooser {
            player,
            from: game.zone(Zone::Hand(player)).to_vec(),
            take,
            text: "Discard a card.".into(),
        }
    }

    /// Runs `pick_cards` for the source on the battlefield with `journal` as the answers given so far.
    fn picks(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        choosers: &[Chooser],
    ) -> Result<Picked, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        pick_cards(&cx, choosers)
    }

    fn hand(game: &GameState, player: PlayerId) -> Vec<ObjectId> {
        game.zone(Zone::Hand(player)).to_vec()
    }

    #[test]
    fn the_first_player_with_a_choice_is_asked_with_the_cards_and_the_count() {
        let game = game();
        let asked = picks(&game, Vec::new(), &[chooser(&game, ME, 2)]).unwrap();
        assert_eq!(
            asked,
            Picked::Ask(Decision {
                player: ME,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source: game.zone(Zone::Battlefield)[0],
                    from: hand(&game, ME),
                    min: 2,
                    max: 2,
                    text: "Discard a card.".into(),
                }),
            })
        );
    }

    #[rstest]
    #[case::nothing_to_take(0)]
    #[case::as_many_cards_as_there_are(3)]
    #[case::more_than_there_are(7)]
    fn a_player_with_nothing_to_choose_takes_what_there_is_and_is_not_asked(#[case] take: usize) {
        let game = game();
        let all = hand(&game, ME);
        let wanted = if take == 0 { Vec::new() } else { all };
        assert_eq!(
            picks(&game, Vec::new(), &[chooser(&game, ME, take)]).unwrap(),
            Picked::Done(vec![(ME, wanted)])
        );
    }

    #[test]
    fn a_player_with_no_cards_takes_none() {
        let game = game();
        let mut empty = chooser(&game, ME, 1);
        empty.from.clear();
        assert_eq!(
            picks(&game, Vec::new(), &[empty]).unwrap(),
            Picked::Done(vec![(ME, Vec::new())])
        );
    }

    #[test]
    fn several_players_are_asked_in_order_and_their_answers_are_read_back_in_order() {
        let game = game();
        // Seat 0 must choose among three, seat 1 has only one card for the one they take.
        let choosers = [chooser(&game, ME, 1), chooser(&game, FOE, 1)];
        let first = hand(&game, ME)[1];
        let done = picks(&game, vec![(ME, Choice::Cards(vec![first]))], &choosers).unwrap();
        assert_eq!(
            done,
            Picked::Done(vec![(ME, vec![first]), (FOE, hand(&game, FOE))])
        );
        // Two players with a choice: the second is asked once the first has answered.
        let mut both = choosers.clone();
        both[1].take = 1;
        both[1].from = ids(&[40, 41]);
        let next = picks(&game, vec![(ME, Choice::Cards(vec![first]))], &both).unwrap();
        let Picked::Ask(decision) = next else {
            panic!("the second player is asked");
        };
        assert_eq!(decision.player, FOE);
        let done = picks(
            &game,
            vec![
                (ME, Choice::Cards(vec![first])),
                (FOE, Choice::Cards(ids(&[41]))),
            ],
            &both,
        )
        .unwrap();
        assert_eq!(
            done,
            Picked::Done(vec![(ME, vec![first]), (FOE, ids(&[41]))])
        );
    }

    #[test]
    fn an_answer_the_step_cannot_use_is_a_bug_named_as_one() {
        let game = game();
        let own = chooser(&game, ME, 1);
        let one = hand(&game, ME)[0];
        let wrong_player = vec![(FOE, Choice::Cards(vec![one]))];
        let wrong_cards = vec![(ME, Choice::Cards(ids(&[77])))];
        let wrong_kind = vec![(ME, Choice::YesNo(true))];
        let extra = vec![
            (ME, Choice::Cards(vec![one])),
            (ME, Choice::Cards(vec![one])),
        ];
        for journal in [wrong_player, wrong_cards, wrong_kind, extra] {
            let refused = picks(&game, journal.clone(), std::slice::from_ref(&own));
            assert!(
                matches!(refused, Err(ResolveError::Invalid(_))),
                "{journal:?}: {refused:?}"
            );
        }
    }
}
