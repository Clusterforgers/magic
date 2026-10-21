//! Goal: +1/+1 and -1/-1 counters change power and toughness in layer 7c (CR 613.4c) whatever else is in play, even in a game with no effect and no static ability: after what sets power and toughness and before what switches them, in step with the other pumps whatever their timestamps, before an effect that reads power or toughness; counters of other kinds change nothing; the result follows the counters as events change them, and goes with the permanent when it leaves.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use proptest::prelude::*;
use rstest::rstest;

use super::tests::{effect, fold, named, pump, rules};
use super::*;
use crate::characteristics::characteristics;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, setup};
use crate::setup::PresetPermanent;
use crate::snapshot::snapshot;

const ME: PlayerId = PlayerId(0);
const PLUS: CounterKind = CounterKind::PlusOnePlusOne;
const MINUS: CounterKind = CounterKind::MinusOneMinusOne;

/// A 0/4 for the tests of switching.
fn wall() -> Arc<CardDefinition> {
    creature("Wall", 0, 4)
}

/// An enchantment that gives creatures with power 3 or more +1/+1: an effect that reads power.
fn banner() -> Arc<CardDefinition> {
    let ability = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":3}}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
    Arc::new(
        CardDefinition::new("Banner")
            .with_type(CardType::Enchantment)
            .with_rules(rules(ability)),
    )
}

/// Seat 0 controls a Bear 2/2 with these counters and a Wall 0/4 with two +1/+1 counters, and the cards given.
fn game_with(counters: &[(CounterKind, u32)], more: Vec<Arc<CardDefinition>>) -> GameState {
    let mut setup = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
    setup.players[0].battlefield = vec![PresetPermanent {
        counters: counters.to_vec(),
        ..PresetPermanent::new(creature("Bear", 2, 2))
    }];
    setup.players[0].battlefield.push(PresetPermanent {
        counters: vec![(PLUS, 2)],
        ..PresetPermanent::new(wall())
    });
    setup.players[0]
        .battlefield
        .extend(more.into_iter().map(PresetPermanent::new));
    GameState::from_setup(&setup).unwrap()
}

fn put(game: &mut GameState, name: &str, counter: CounterKind, delta: i32) {
    let on = Target::Object(named(game, name));
    fold(
        game,
        &[Event::CountersChanged(CountersChanged {
            on,
            counter,
            delta,
        })],
    );
}

#[rstest]
#[case::none(&[], (2, 2))]
#[case::two_plus_counters(&[(PLUS, 2)], (4, 4))]
#[case::a_minus_counter(&[(MINUS, 1)], (1, 1))]
#[case::minus_counters_below_zero(&[(MINUS, 3)], (-1, -1))]
#[case::both_kinds_before_the_state_based_action(&[(PLUS, 2), (MINUS, 1)], (3, 3))]
#[case::a_kind_that_changes_nothing(&[(CounterKind::Charge, 4), (CounterKind::Poison, 1)], (2, 2))]
fn counters_change_power_and_toughness_in_a_game_with_nothing_else_in_play(
    #[case] counters: &[(CounterKind, u32)],
    #[case] expected: (i32, i32),
) {
    let game = game_with(counters, vec![]);
    assert!(game.effects.is_empty() && !game.statics_possible);
    assert_eq!(pump(&game, "Bear"), Some(expected));
}

#[test]
fn a_game_without_such_counters_works_out_nothing() {
    let setup = setup(
        vec![vec![], vec![]],
        vec![vec![creature("Bear", 2, 2)], vec![]],
    );
    let game = GameState::from_setup(&setup).unwrap();
    assert!(!game.possible.counters);
    assert!(game.layers().get(named(&game, "Bear").id).is_none());
    assert_eq!(pump(&game, "Bear"), Some((2, 2)));
}

#[rstest]
#[case::a_set_comes_first(Modification::SetPowerToughness { power: 1, toughness: 1 }, 7, (3, 3))]
#[case::a_set_with_an_earlier_timestamp(Modification::SetPowerToughness { power: 5, toughness: 1 }, 0, (7, 3))]
#[case::a_pump_of_a_later_timestamp(Modification::ModifyPowerToughness { power: 2, toughness: -1 }, 9, (6, 3))]
#[case::a_pump_of_an_earlier_timestamp(Modification::ModifyPowerToughness { power: 2, toughness: -1 }, 0, (6, 3))]
fn counters_are_layer_seven_c_among_the_other_effects(
    #[case] modification: Modification,
    #[case] timestamp: u64,
    #[case] expected: (i32, i32),
) {
    let mut game = game_with(&[(PLUS, 2)], vec![]);
    effect(&mut game, "Bear", modification, timestamp);
    assert_eq!(pump(&game, "Bear"), Some(expected));
}

#[test]
fn counters_are_applied_before_a_switch_and_after_a_set() {
    // The Wall is 0/4 with two +1/+1 counters: 2/6, and a switch makes it 6/2.
    let mut game = game_with(&[], vec![]);
    assert_eq!(pump(&game, "Wall"), Some((2, 6)));
    effect(&mut game, "Wall", Modification::SwitchPowerToughness, 1);
    assert_eq!(pump(&game, "Wall"), Some((6, 2)));
    // A set to 1/1 leaves the counters to add to it, and the switch still comes last.
    effect(
        &mut game,
        "Wall",
        Modification::SetPowerToughness {
            power: 1,
            toughness: 3,
        },
        2,
    );
    assert_eq!(pump(&game, "Wall"), Some((5, 3)));
}

#[test]
fn counters_apply_before_an_effect_that_reads_power() {
    // The Bear is 2/2 and the banner helps creatures with power 3 or more: the counter makes the Bear one of them.
    let without = game_with(&[], vec![banner()]);
    assert_eq!(pump(&without, "Bear"), Some((2, 2)));
    let with = game_with(&[(PLUS, 1)], vec![banner()]);
    assert_eq!(
        pump(&with, "Bear"),
        Some((4, 4)),
        "3/3 with the counter, then the banner"
    );
    // The Wall has power 2 with its counters and is left alone.
    assert_eq!(pump(&with, "Wall"), Some((2, 6)));
}

#[test]
fn the_result_follows_the_counters_as_events_change_them() {
    let mut game = game_with(&[], vec![]);
    assert_eq!(pump(&game, "Bear"), Some((2, 2)));
    put(&mut game, "Bear", PLUS, 3);
    assert_eq!(pump(&game, "Bear"), Some((5, 5)));
    put(&mut game, "Bear", MINUS, 1);
    assert_eq!(pump(&game, "Bear"), Some((4, 4)));
    put(&mut game, "Bear", PLUS, -3);
    assert_eq!(pump(&game, "Bear"), Some((1, 1)));
    put(&mut game, "Bear", MINUS, -1);
    assert_eq!(pump(&game, "Bear"), Some((2, 2)));
    put(&mut game, "Bear", CounterKind::Charge, 2);
    assert_eq!(pump(&game, "Bear"), Some((2, 2)));
}

#[test]
fn a_game_that_starts_with_none_looks_once_one_is_put_on() {
    let setup = setup(
        vec![vec![], vec![]],
        vec![vec![creature("Bear", 2, 2)], vec![]],
    );
    let mut game = GameState::from_setup(&setup).unwrap();
    put(&mut game, "Bear", PLUS, 1);
    assert!(game.possible.counters);
    assert_eq!(pump(&game, "Bear"), Some((3, 3)));
    // The flag stays up when the counter goes again, and the result is the card's own.
    put(&mut game, "Bear", PLUS, -1);
    assert_eq!(pump(&game, "Bear"), Some((2, 2)));
}

#[test]
fn counters_do_not_follow_a_permanent_that_left_and_came_back() {
    let mut game = game_with(&[(PLUS, 2)], vec![]);
    let bear = named(&game, "Bear").id;
    assert_eq!(pump(&game, "Bear"), Some((4, 4)));
    fold(
        &mut game,
        &[
            Event::ZoneChanged {
                object: bear,
                from: Zone::Battlefield,
                to: Zone::Hand(ME),
                cause: ZoneCause::Moved,
            },
            Event::ZoneChanged {
                object: bear,
                from: Zone::Hand(ME),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            },
        ],
    );
    assert_eq!(pump(&game, "Bear"), Some((2, 2)));
}

#[test]
fn a_snapshot_keeps_the_power_and_toughness_the_counters_gave() {
    let game = game_with(&[(PLUS, 2), (MINUS, 1)], vec![]);
    let shot = snapshot(&game, named(&game, "Bear").id).unwrap();
    assert_eq!(
        (shot.characteristics.power, shot.characteristics.toughness),
        (Some(3), Some(3))
    );
    assert_eq!(shot.counters, [(PLUS, 2), (MINUS, 1)]);
}

#[test]
fn a_permanent_without_power_and_toughness_is_not_given_any() {
    let relic = Arc::new(CardDefinition::new("Relic").with_type(CardType::Artifact));
    let mut game = game_with(&[], vec![relic]);
    put(&mut game, "Relic", PLUS, 2);
    let chars = characteristics(&game, named(&game, "Relic").id).unwrap();
    assert_eq!((chars.power, chars.toughness), (None, None));
}

proptest! {
    /// Whatever the counters, a creature's power and toughness are its card's plus its +1/+1 counters less its -1/-1
    /// counters, with or without a set before them and a pump beside them.
    #[test]
    fn power_and_toughness_are_the_card_plus_the_pluses_less_the_minuses(
        pluses in 0u32..30,
        minuses in 0u32..30,
        set in proptest::option::of((-3i32..8, -3i32..8)),
        pump_by in -5i32..=5,
        timestamp in 0u64..6,
    ) {
        let mut game = game_with(&[(PLUS, pluses), (MINUS, minuses)], vec![]);
        let net = i32::try_from(pluses).unwrap() - i32::try_from(minuses).unwrap();
        let (mut power, mut toughness) = (2, 2);
        if let Some((p, t)) = set {
            effect(&mut game, "Bear", Modification::SetPowerToughness { power: p, toughness: t }, timestamp);
            (power, toughness) = (p, t);
        }
        effect(
            &mut game,
            "Bear",
            Modification::ModifyPowerToughness { power: pump_by, toughness: pump_by },
            timestamp + 1,
        );
        prop_assert_eq!(
            pump(&game, "Bear"),
            Some((power + net + pump_by, toughness + net + pump_by))
        );
    }
}

#[test]
fn a_huge_number_of_counters_does_not_overflow() {
    let mut game = game_with(&[], vec![]);
    for _ in 0..2 {
        put(&mut game, "Bear", PLUS, i32::MAX);
    }
    let (power, toughness) = pump(&game, "Bear").unwrap();
    assert_eq!((power, toughness), (i32::MAX, i32::MAX));
}
