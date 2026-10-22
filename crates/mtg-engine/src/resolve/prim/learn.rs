//! `Effect::Learn`: a player learns (CR 701.48).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::{Questions, Stop};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::steps::draw;
use crate::zones::Zone;

/// Each player the effect names who is still in the game learns in turn: one `Learned`, then the player may discard a
/// card (asked with `ChooseCards` over the hand, none or one, and not at all with an empty hand) and draws a card if
/// they do. A Lesson card from outside the game is not an option: a Commander game has no sideboard.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Learn { who } = effect else {
        return Err(wrong_effect("Learn"));
    };
    Stop::settle(learn(cx, who))
}

fn learn(cx: &Cx<'_>, who: &mtg_cards::refs::Ref) -> Result<Run, Stop> {
    let state = cx.state();
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) {
            continue;
        }
        events.push(Event::Learned { player });
        let hand = state.zone(Zone::Hand(player)).to_vec();
        if hand.is_empty() {
            continue;
        }
        let text = "You may discard a card. If you do, draw a card.".to_owned();
        if let Some(card) = questions.cards(player, hand, (0, 1), text)?.first() {
            events.push(Event::CardDiscarded {
                player,
                object: *card,
            });
            events.extend(draw(state, player, 1));
        }
    }
    questions.end()?;
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: learning logs `Learned` and asks the player which card to discard, none or one; a card chosen is discarded and followed by a draw, no card chosen is no discard and no draw, an empty hand is not asked, and another effect is refused.
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::choose::Choice;
    use crate::fixtures::{creature, setup};
    use crate::resolve::{Resolving, Scope};
    use crate::state::GameState;

    const ME: PlayerId = PlayerId(0);

    fn game(hand: usize) -> GameState {
        let cards = (0..hand)
            .map(|n| creature(&format!("Card {n}"), 1, 1))
            .collect();
        let board = vec![creature("Source", 1, 1)];
        GameState::from_setup(&setup(vec![cards, vec![]], vec![board, vec![]])).unwrap()
    }

    fn run_with(game: &GameState, journal: Vec<(PlayerId, Choice)>) -> Result<Run, ResolveError> {
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving {
            journal,
            ..Resolving::begin(source.id())
        };
        let cx = Cx {
            scope: Scope::announcing(game, ME, source.reference()),
            resolving: &resolving,
        };
        run(&cx, &serde_json::from_str(r#"{"Learn":{}}"#).unwrap())
    }

    fn events(result: Result<Run, ResolveError>) -> Vec<Event> {
        match result.unwrap() {
            Run::Done { events, .. } => events,
            other => panic!("done, got {other:?}"),
        }
    }

    fn first_card(game: &GameState) -> ObjectId {
        game.zone(Zone::Hand(ME))[0]
    }

    #[test]
    fn a_hand_with_cards_is_asked_which_to_discard() {
        let game = game(2);
        assert!(matches!(
            run_with(&game, vec![]),
            Ok(Run::Ask(decision)) if decision.player == ME
        ));
    }

    #[rstest]
    #[case::discard_one(true, 3)]
    #[case::decline(false, 1)]
    fn the_answer_decides_the_discard_and_the_draw(
        #[case] discards: bool,
        #[case] expected: usize,
    ) {
        let game = game(2);
        let chosen = if discards {
            vec![first_card(&game)]
        } else {
            vec![]
        };
        let found = events(run_with(&game, vec![(ME, Choice::Cards(chosen))]));
        assert_eq!(found.len(), expected);
        assert_eq!(found[0], Event::Learned { player: ME });
        if discards {
            assert!(matches!(found[1], Event::CardDiscarded { player, .. } if player == ME));
            assert!(matches!(found[2], Event::CardDrawn { player, .. } if player == ME));
        }
    }

    #[test]
    fn an_empty_hand_only_learns() {
        let game = game(0);
        assert_eq!(
            events(run_with(&game, vec![])),
            [Event::Learned { player: ME }]
        );
    }

    #[test]
    fn another_effect_is_refused() {
        let game = game(1);
        let source = game.objects().find(|o| o.card().name == "Source").unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let other: Effect = serde_json::from_str(r#"{"Adapt":{"count":1}}"#).unwrap();
        assert!(run(&cx, &other).is_err());
    }
}
