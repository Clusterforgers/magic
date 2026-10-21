//! `Effect::Discard`: players discard cards of their choice (CR 701.9).

use mtg_cards::effect::Effect;

use super::{unsupported, wrong_effect};
use crate::choose::{Chooser, Picked, pick_cards};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::wording;
use crate::zones::Zone;

/// Each player the effect names who is still in the game discards the number of cards from their hand, in turn
/// order. The player chooses which and is asked only when the hand holds more than they discard; a smaller hand is
/// discarded whole, and an empty one is no event. A random discard is not played yet and `Caps` does not name it.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Discard { who, count, random } = effect else {
        return Err(wrong_effect("Discard"));
    };
    if *random {
        return unsupported("Effect.Discard.Random");
    }
    let take = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let choosers: Vec<Chooser> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| cx.state().is_alive(*player))
        .map(|player| Chooser {
            player,
            from: cx.state().zone(Zone::Hand(player)).to_vec(),
            take,
            text: wording::discard(take),
        })
        .collect();
    Ok(match pick_cards(cx, &choosers)? {
        Picked::Ask(decision) => Run::Ask(decision),
        Picked::Done(chosen) => Run::events(
            chosen
                .into_iter()
                .flat_map(|(player, cards)| {
                    cards
                        .into_iter()
                        .map(move |object| Event::CardDiscarded { player, object })
                })
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a discard takes the number of cards from each player it names in turn order, the player chooses and is asked only when the hand holds more than that, a smaller or empty hand is discarded without a question, the question carries the words of the card, and what cannot be played yet is refused by name.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::{CardsOptions, Choice};
    use crate::decision::{Decision, DecisionKind};
    use crate::fixtures::{creature, state};
    use crate::resolve::{Next, Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    /// Seat 0 holds A, B and C, seat 1 holds D and E; the Source is on the battlefield.
    fn game() -> GameState {
        state(
            vec![
                vec![
                    creature("A", 1, 1),
                    creature("B", 1, 1),
                    creature("C", 1, 1),
                ],
                vec![creature("D", 1, 1), creature("E", 1, 1)],
            ],
            vec![vec![creature("Source", 1, 1)], vec![]],
        )
    }

    fn effect(json: &str) -> Effect {
        serde_json::from_str(json).unwrap()
    }

    fn hand(game: &GameState, player: PlayerId) -> Vec<ObjectId> {
        game.zone(Zone::Hand(player)).to_vec()
    }

    fn run_with(
        game: &GameState,
        journal: Vec<(PlayerId, Choice)>,
        effect: &Effect,
    ) -> Result<Run, ResolveError> {
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, effect)
    }

    /// The discards a finished run proposes, as (who, which).
    fn discards(run: Run) -> Vec<(PlayerId, ObjectId)> {
        let Run::Done {
            events,
            bound,
            snapshots,
            next,
        } = run
        else {
            panic!("the effect is done");
        };
        assert!(bound.is_empty() && snapshots.is_empty() && next == Next::Sibling);
        events
            .into_iter()
            .map(|event| match event {
                Event::CardDiscarded { player, object } => (player, object),
                other => panic!("only discards were expected, got {other:?}"),
            })
            .collect()
    }

    fn asked(run: Run) -> Decision {
        let Run::Ask(decision) = run else {
            panic!("the effect asks");
        };
        decision
    }

    #[test]
    fn the_player_is_asked_for_the_cards_when_the_hand_holds_more_than_is_discarded() {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), &effect(r#"{"Discard":{}}"#)).unwrap());
        assert_eq!(
            decision,
            Decision {
                player: ME,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source: game.zone(Zone::Battlefield)[0],
                    from: hand(&game, ME),
                    min: 1,
                    max: 1,
                    text: "Discard a card.".into(),
                    can_cancel: false,
                }),
            }
        );
    }

    #[test]
    fn the_answer_decides_which_cards_go() {
        let game = game();
        let picked = hand(&game, ME)[1];
        let run = run_with(
            &game,
            vec![(ME, Choice::Cards(vec![picked]))],
            &effect(r#"{"Discard":{}}"#),
        )
        .unwrap();
        assert_eq!(discards(run), [(ME, picked)]);
    }

    #[rstest]
    #[case::the_target_of_the_ability(r#"{"Discard":{"who":"Opponents"}}"#, FOE, "Discard a card.")]
    #[case::two_cards(r#"{"Discard":{"count":2}}"#, ME, "Discard 2 cards.")]
    fn any_seat_may_be_the_one_asked_and_the_count_is_in_the_words(
        #[case] json: &str,
        #[case] chooser: PlayerId,
        #[case] text: &str,
    ) {
        let game = game();
        let decision = asked(run_with(&game, Vec::new(), &effect(json)).unwrap());
        assert_eq!(decision.player, chooser);
        let DecisionKind::ChooseCards(options) = decision.kind else {
            panic!("cards are chosen");
        };
        assert_eq!(options.from, hand(&game, chooser));
        assert_eq!(options.text, text);
    }

    #[rstest]
    #[case::the_whole_hand_when_it_is_no_larger(
        r#"{"Discard":{"who":"Opponents","count":2}}"#,
        FOE
    )]
    #[case::the_whole_hand_when_it_is_smaller(r#"{"Discard":{"who":"Opponents","count":5}}"#, FOE)]
    fn a_hand_no_larger_than_the_count_is_discarded_without_a_question(
        #[case] json: &str,
        #[case] player: PlayerId,
    ) {
        let game = game();
        let run = run_with(&game, Vec::new(), &effect(json)).unwrap();
        let all = hand(&game, player);
        assert_eq!(
            discards(run),
            all.into_iter().map(|c| (player, c)).collect::<Vec<_>>()
        );
    }

    #[rstest]
    #[case::none_to_discard(r#"{"Discard":{"count":0}}"#)]
    #[case::a_negative_count(r#"{"Discard":{"count":-2}}"#)]
    fn discarding_nothing_asks_nothing_and_happens_nothing(#[case] json: &str) {
        let game = game();
        let run = run_with(&game, Vec::new(), &effect(json)).unwrap();
        assert_eq!(discards(run), []);
    }

    #[test]
    fn an_empty_hand_has_nothing_to_discard_and_nothing_is_asked() {
        let game = state(
            vec![vec![], vec![creature("D", 1, 1)]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        let run = run_with(&game, Vec::new(), &effect(r#"{"Discard":{}}"#)).unwrap();
        assert_eq!(discards(run), []);
    }

    #[test]
    fn each_player_is_asked_in_turn_order_and_the_discards_follow_together() {
        let game = game();
        let each = effect(r#"{"Discard":{"who":"EachPlayer"}}"#);
        // Seat 0 is asked first; seat 1 holds two cards, so seat 1 is asked next.
        let first = asked(run_with(&game, Vec::new(), &each).unwrap());
        assert_eq!(first.player, ME);
        let mine = hand(&game, ME)[2];
        let second = asked(run_with(&game, vec![(ME, Choice::Cards(vec![mine]))], &each).unwrap());
        assert_eq!(second.player, FOE);
        let theirs = hand(&game, FOE)[0];
        let run = run_with(
            &game,
            vec![
                (ME, Choice::Cards(vec![mine])),
                (FOE, Choice::Cards(vec![theirs])),
            ],
            &each,
        )
        .unwrap();
        assert_eq!(discards(run), [(ME, mine), (FOE, theirs)]);
    }

    #[test]
    fn a_player_who_has_left_the_game_discards_nothing() {
        let mut game = game();
        crate::fold::apply_event(
            &mut game,
            &Event::PlayerLost {
                player: FOE,
                reason: crate::player::LossReason::Conceded,
            },
        )
        .unwrap();
        let run = run_with(
            &game,
            Vec::new(),
            &effect(r#"{"Discard":{"who":"Opponents","count":2}}"#),
        )
        .unwrap();
        assert_eq!(discards(run), []);
    }

    #[test]
    fn an_answer_that_names_a_card_outside_the_hand_is_a_bug() {
        let game = game();
        let foreign = hand(&game, FOE)[0];
        let refused = run_with(
            &game,
            vec![(ME, Choice::Cards(vec![foreign]))],
            &effect(r#"{"Discard":{}}"#),
        );
        assert!(
            matches!(refused, Err(ResolveError::Invalid(_))),
            "{refused:?}"
        );
    }

    #[test]
    fn a_random_discard_is_refused_by_name() {
        let game = game();
        let refused = run_with(&game, Vec::new(), &effect(r#"{"Discard":{"random":true}}"#));
        assert_eq!(
            refused.unwrap_err(),
            ResolveError::Unsupported("Effect.Discard.Random")
        );
    }

    #[test]
    fn another_effect_is_a_bug_in_the_dispatch() {
        let game = game();
        let refused = run_with(&game, Vec::new(), &effect(r#"{"Draw":{}}"#));
        assert!(matches!(refused, Err(ResolveError::Invalid(_))));
    }
}
