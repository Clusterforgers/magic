//! Goal: a coin flip reaches the wire with the call and the side it came up for every combination, a call is offered as the sides of a coin, and a flip logged before calls existed still reads (as heads called and heads come up).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::{ChoiceOption, CoinSide};
use rstest::rstest;
use serde_json::json;

#[rstest]
fn a_flip_carries_its_call_and_its_side(
    #[values(CoinSide::Heads, CoinSide::Tails)] call: CoinSide,
    #[values(CoinSide::Heads, CoinSide::Tails)] side: CoinSide,
) {
    let event = Event::CoinFlipped {
        player: PlayerId(1),
        won: call == side,
        call,
        side,
    };
    let wire = serde_json::to_value(&event).unwrap();
    assert_eq!(
        wire,
        json!({"CoinFlipped": {
            "player": 1,
            "won": call == side,
            "call": format!("{call:?}"),
            "side": format!("{side:?}"),
        }})
    );
    assert_eq!(serde_json::from_value::<Event>(wire).unwrap(), event);
}

#[test]
fn a_side_is_offered_by_its_name() {
    assert_eq!(
        serde_json::to_value(ChoiceOption::Side(CoinSide::Tails)).unwrap(),
        json!({"Side": "Tails"})
    );
}

#[test]
fn a_flip_logged_without_a_call_reads_as_heads() {
    let event: Event =
        serde_json::from_value(json!({"CoinFlipped": {"player": 0, "won": true}})).unwrap();
    assert_eq!(
        event,
        Event::CoinFlipped {
            player: PlayerId(0),
            won: true,
            call: CoinSide::Heads,
            side: CoinSide::Heads,
        }
    );
}
