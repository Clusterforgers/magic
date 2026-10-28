//! Goal: a mana ability that puts a counter on its source before the mana that counts it ("Put a charge counter on this artifact. Add X mana of any one color, where X is the number of charge counters on this artifact.") offers the mana counted with that counter, and its use puts the counter first, then adds that many mana of the chosen color, at any table size, and the same events replayed give the same state.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use rstest::rstest;

use super::*;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::fixtures::state;
use crate::fold::apply_event;

const ME: PlayerId = PlayerId(0);

const C8_TIME_AUTOGENERATOR: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"PutCounters":{"on":"This","counter":"charge"}},{"AddMana":{"mana":{"AnyColor":{"Counters":{"on":"This","counter":"charge"}}}}}]}}"#;

/// A table of `seats` where seat 0 controls the generator with `held` charge counters.
fn game(seats: usize, held: i32) -> GameState {
    let generator = CardDefinition::new("Generator")
        .with_type(CardType::Artifact)
        .with_rules(serde_json::from_str(C8_TIME_AUTOGENERATOR).unwrap());
    let mut boards = vec![Vec::new(); seats];
    boards[0] = vec![Arc::new(generator)];
    let mut game = state(vec![Vec::new(); seats], boards);
    let id = game.zone(Zone::Battlefield)[0];
    if held > 0 {
        let on = Target::Object(game.object(id).unwrap().reference());
        let changed = CountersChanged {
            on,
            counter: CounterKind::Charge,
            delta: held,
        };
        apply_event(&mut game, &Event::CountersChanged(changed)).unwrap();
    }
    game
}

#[rstest]
#[case::two_seats_none_held(2, 0)]
#[case::three_seats_one_held(3, 1)]
#[case::four_seats_three_held(4, 3)]
fn the_counter_goes_on_first_and_the_mana_counts_it(#[case] seats: usize, #[case] held: i32) {
    let game = game(seats, held);
    let id = game.zone(Zone::Battlefield)[0];
    let offered = options(&game, ME);
    let mine: Vec<&ManaOption> = offered.iter().filter(|o| o.source.object == id).collect();
    assert_eq!(mine.len(), COLORS.len(), "one source for each color");
    let count = u32::try_from(held + 1).unwrap();
    assert!(mine.iter().all(|o| o.count == count && o.extra));
    let events = activation(&game, ME, mine[0], &[]).unwrap();
    // The cost (the tap) is paid, then the counter goes on before any mana.
    let Some(Event::CountersChanged(first)) = events.get(1) else {
        panic!("the counter goes on after the tap: {events:?}");
    };
    assert!(matches!(events[0], Event::PermanentTapped { .. }));
    assert_eq!((first.counter.clone(), first.delta), (CounterKind::Charge, 1));
    let added = events
        .iter()
        .filter(|e| matches!(e, Event::ManaAdded { .. }))
        .count();
    assert_eq!(added, count as usize);
    let mut after = game.clone();
    let mut replay = game.clone();
    for event in &events {
        apply_event(&mut after, event).unwrap();
        apply_event(&mut replay, event).unwrap();
    }
    let on = after.object(id).unwrap().permanent().unwrap();
    assert_eq!(on.counters_of(&CounterKind::Charge), count);
    assert!(on.tapped());
    assert_eq!(after.state_hash(), replay.state_hash());
    assert_ne!(after.state_hash(), game.state_hash());
}
