//! `Effect::LookAtHand`: the controller looks at the whole hand of a player.

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::library::HandLooked;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The controller looks at the hand of each player the effect names, in turn order. Each hand is one `HandLooked`
/// (only the controller learns the cards; the others see how many). An empty hand is no event.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::LookAtHand { of } = effect else {
        return Err(wrong_effect("LookAtHand"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    for owner in cx.scope.players(of)? {
        let cards = state.zone(Zone::Hand(owner)).to_vec();
        if !cards.is_empty() {
            events.push(Event::HandLooked(HandLooked {
                player: cx.controller(),
                of: owner,
                cards,
            }));
        }
    }
    Ok(Run::events(events))
}

#[cfg(test)]
mod tests {
    //! Goal: looking at a hand names every card of each hand the effect names to the controller alone, one event per hand in turn order, whoever the hand belongs to, and is no event for an empty hand.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};
    use crate::resolve::{Resolving, Scope};

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    #[rstest]
    #[case::the_opponent(r#"{"LookAtHand":{"of":"Opponents"}}"#, &[(FOE, 2)])]
    #[case::your_own(r#"{"LookAtHand":{"of":"You"}}"#, &[(ME, 1)])]
    #[case::every_player(r#"{"LookAtHand":{"of":"EachPlayer"}}"#, &[(ME, 1), (FOE, 2)])]
    fn each_hand_is_one_event_for_the_controller(
        #[case] json: &str,
        #[case] expected: &[(PlayerId, usize)],
    ) {
        let game = state(
            vec![
                vec![creature("Mine", 1, 1)],
                vec![creature("A", 1, 1), creature("B", 1, 1)],
            ],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let scope = Scope::announcing(&game, ME, source.reference());
        let cx = Cx {
            scope,
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(json).unwrap();
        let Ok(run) = run(&cx, &effect) else {
            panic!("a look at a hand runs");
        };
        let Run::Done { events, .. } = run else {
            panic!("a look at a hand finishes");
        };
        let got: Vec<(PlayerId, usize)> = events
            .iter()
            .map(|event| match event {
                Event::HandLooked(looked) => {
                    assert_eq!(looked.player, ME);
                    (looked.of, looked.cards.len())
                }
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn an_empty_hand_is_no_event() {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Source", 1, 1)], vec![]],
        );
        let source = game.object(game.zone(Zone::Battlefield)[0]).unwrap();
        let resolving = Resolving::begin(source.id());
        let cx = Cx {
            scope: Scope::announcing(&game, ME, source.reference()),
            resolving: &resolving,
        };
        let effect: Effect = serde_json::from_str(r#"{"LookAtHand":{"of":"Opponents"}}"#).unwrap();
        let Ok(Run::Done { events, .. }) = run(&cx, &effect) else {
            panic!("a look at a hand finishes");
        };
        assert_eq!(events.len(), 0);
    }
}
