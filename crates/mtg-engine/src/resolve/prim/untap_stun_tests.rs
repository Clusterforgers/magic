//! Goal: an untap passes a permanent with a stun counter over and removes one counter from it in its place (CR 122.1g), whatever the number of counters, and untaps the ones without a counter as before.
use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use crate::choose::Target;
use crate::events::Event;
use crate::fixtures::{creature, setup};
use crate::resolve::{Cx, Resolving, Run, Scope};
use crate::state::GameState;

/// Seat 0 controls the Source, a tapped Wolf with `stun` stun counters and a tapped Bear.
fn game(stun: u32) -> GameState {
    let mut game = setup(
        vec![vec![], vec![]],
        vec![
            vec![
                creature("Source", 1, 1),
                creature("Wolf", 2, 2),
                creature("Bear", 2, 2),
            ],
            vec![],
        ],
    );
    game.players[0].battlefield[1].tapped = true;
    game.players[0].battlefield[2].tapped = true;
    if stun > 0 {
        game.players[0].battlefield[1].counters = vec![(crate::stun::kind(), stun)];
    }
    GameState::from_setup(&game).unwrap()
}

#[rstest]
#[case::no_counter(0, &["untap Wolf", "untap Bear"])]
#[case::one_counter(1, &["unstun Wolf", "untap Bear"])]
#[case::two_counters(2, &["unstun Wolf", "untap Bear"])]
fn a_stunned_permanent_loses_a_counter_in_place_of_untapping(
    #[case] stun: u32,
    #[case] expected: &[&str],
) {
    let game = game(stun);
    let source = game
        .objects()
        .find(|o| o.card().name == "Source")
        .map(crate::object::GameObject::reference)
        .unwrap();
    let resolving = Resolving::begin(source.id);
    let cx = Cx {
        scope: Scope::announcing(&game, PlayerId(0), source),
        resolving: &resolving,
    };
    let effect: Effect = serde_json::from_str(r#"{"Untap":{"what":{"Each":["Tapped"]}}}"#).unwrap();
    let Run::Done { events, .. } = super::untap::run(&cx, &effect).unwrap() else {
        panic!("an untap needs no answer");
    };
    let said: Vec<String> = events
        .iter()
        .map(|event| match event {
            Event::PermanentUntapped { object } => {
                format!("untap {}", game.object(*object).unwrap().card().name)
            }
            Event::CountersChanged(changed) => {
                assert_eq!(
                    (changed.delta, &changed.counter),
                    (-1, &crate::stun::kind())
                );
                let Target::Object(on) = changed.on else {
                    panic!("a permanent was expected");
                };
                format!("unstun {}", game.object(on.id).unwrap().card().name)
            }
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(said, expected);
}
