//! Goal: the mana abilities of round 6 builder e play: "X plus one" of a battery is a source for each X from none up to the counters held, a depletion land is sacrificed only when its last counter is removed, and a rider under a condition runs only when it holds, at two and three seats.
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

/// "{T}, Remove any number of charge counters: Add {B}, then add an additional {B} for each charge counter removed".
const BATTERY: &str = r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"count":"X","counter":"charge"}}],"effects":[{"AddMana":{"mana":{"Fixed":["Black"]}}},{"AddMana":{"mana":{"Fixed":["Black"]},"times":"X"}}]}}"#;
/// "{T}, Remove X charge counters: Add an amount of {C} equal to X plus one."
const TOY: &str = r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"count":"X","counter":"charge"}}],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Sum":["X",1]}}}]}}"#;
/// "{T}, Remove a depletion counter: Add {B}{B}. If there are no depletion counters on this land, sacrifice it."
const DEPLETION: &str = r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":{"Other":"depletion"}}}],"effects":[{"AddMana":{"mana":{"Fixed":["Black","Black"]}}},{"If":{"condition":{"Compare":{"left":{"Count":["This",{"Not":{"HasCounter":{"Other":"depletion"}}}]},"op":"Ge","right":1}},"then":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#;
/// "{T}: Add {G}. If you control an artifact, you gain 2 life."
const RIDER: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"then":[{"GainLife":{"amount":2}}]}}]}}"#;

/// A game of `seats` players where the first has a land with `json` and `counters` counters of `kind` on it, and the
/// `others` beside it.
fn game(
    seats: usize,
    json: &str,
    kind: &CounterKind,
    counters: i32,
    others: Vec<Arc<CardDefinition>>,
) -> GameState {
    let land = CardDefinition::new("Source")
        .with_type(CardType::Land)
        .with_rules(serde_json::from_str(json).unwrap());
    let mut permanents = vec![Arc::new(land)];
    permanents.extend(others);
    let mut battlefield = vec![permanents];
    battlefield.resize(seats, Vec::new());
    let mut game = state(vec![Vec::new(); seats], battlefield);
    let id = game.zone(Zone::Battlefield)[0];
    if counters > 0 {
        let on = Target::Object(game.object(id).unwrap().reference());
        let changed = CountersChanged {
            on,
            counter: kind.clone(),
            delta: counters,
        };
        apply_event(&mut game, &Event::CountersChanged(changed)).unwrap();
    }
    game
}

fn charge() -> CounterKind {
    CounterKind::Charge
}

fn depletion() -> CounterKind {
    CounterKind::Other("depletion".into())
}

fn source(game: &GameState) -> Vec<ManaOption> {
    let id = game.zone(Zone::Battlefield)[0];
    options(game, ME)
        .into_iter()
        .filter(|option| option.source.object == id)
        .collect()
}

fn removed(events: &[Event]) -> Vec<i32> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::CountersChanged(changed) => Some(changed.delta),
            _ => None,
        })
        .collect()
}

fn sacrificed(events: &[Event]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            Event::ZoneChanged {
                cause: ZoneCause::Sacrificed,
                ..
            }
        )
    })
}

#[rstest]
fn x_plus_one_is_a_source_for_each_x_from_none_to_the_counters(
    #[values(BATTERY, TOY)] json: &str,
    #[values(0, 1, 3)] counters: i32,
    #[values(2, 3)] seats: usize,
) {
    let game = game(seats, json, &charge(), counters, Vec::new());
    let found = source(&game);
    let mut made: Vec<usize> = found.iter().map(|o| o.made().len()).collect();
    made.sort_unstable();
    let wanted: Vec<usize> = (1..=usize::try_from(counters).unwrap() + 1).collect();
    assert_eq!(made, wanted);
    for option in &found {
        let x = i32::try_from(option.made().len()).unwrap() - 1;
        let events = activation(&game, ME, option, &[]).unwrap();
        let taken = removed(&events);
        assert_eq!(taken, if x == 0 { vec![] } else { vec![-x] }, "x = {x}");
        let added = events
            .iter()
            .filter(|event| matches!(event, Event::ManaAdded { .. }))
            .count();
        assert_eq!(added, option.made().len());
    }
}

#[rstest]
#[case::two_left(2, false)]
#[case::the_last(1, true)]
fn a_depletion_land_is_sacrificed_when_its_last_counter_is_removed(
    #[case] counters: i32,
    #[case] gone: bool,
    #[values(2, 3)] seats: usize,
) {
    let game = game(seats, DEPLETION, &depletion(), counters, Vec::new());
    let found = source(&game);
    assert_eq!(found.len(), 1);
    let events = activation(&game, ME, &found[0], &[]).unwrap();
    assert_eq!(removed(&events), [-1]);
    assert_eq!(sacrificed(&events), gone);
    assert_eq!(found[0].made().len(), 2);
}

#[test]
fn a_depletion_land_without_counters_is_not_offered() {
    assert_eq!(
        source(&game(2, DEPLETION, &depletion(), 0, Vec::new())).len(),
        0
    );
}

#[rstest]
#[case::with_an_artifact(true, 2)]
#[case::without(false, 0)]
fn a_rider_under_a_condition_runs_only_when_it_holds(#[case] artifact: bool, #[case] life: u32) {
    let others = if artifact {
        vec![Arc::new(
            CardDefinition::new("Bauble").with_type(CardType::Artifact),
        )]
    } else {
        Vec::new()
    };
    let game = game(2, RIDER, &charge(), 0, others);
    let found = source(&game);
    let events = activation(&game, ME, &found[0], &[]).unwrap();
    let gained: u32 = events
        .iter()
        .filter_map(|event| match event {
            Event::LifeGained { player, amount } if *player == ME => Some(*amount),
            _ => None,
        })
        .sum();
    assert_eq!(gained, life);
}
