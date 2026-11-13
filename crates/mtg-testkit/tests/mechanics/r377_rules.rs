//! Goal: "assigns combat damage as though his power were positive" (Loot) deals the absolute value of a negative power
//! and nothing more for a positive one, "as long as you have 0 or less life, all damage is dealt to you as though its
//! source had infect" (Phyrexian Unlife) turns damage into poison counters only while the life is 0 or less, and "each
//! opponent can't venture into the dungeon more than once each turn" (Keen-Eared Sentry) stops an opponent's second
//! venture of a turn but not its controller's, at two to four seats; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{burn, mountain, w11d_venturer};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The log of the driver replays to its state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// A creature of the power, with Loot's rule when `positive`.
fn imp(power: i32, positive: bool) -> Arc<CardDefinition> {
    let mut imp = CardDefinition::new("Imp")
        .with_type(CardType::Creature)
        .with_power_toughness(power, 3);
    if positive {
        let rule: Ability = serde_json::from_str(
            r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":"AssignsAsThoughPositive"}}}}"#,
        )
        .unwrap();
        imp = imp.with_rules(rule);
    }
    Arc::new(imp)
}

#[rstest]
fn r377_negative_power_deals_its_absolute_value(
    #[values(2, 3, 4)] seats: usize,
    #[values(-3, -1, 0, 2)] power: i32,
    #[values(false, true)] positive: bool,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(imp(power, positive))]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let before = driver.state().player(seat(1)).unwrap().life();
    driver.attack(&[("Imp", 1)]).unwrap();
    driver.pass_until(Step::EndCombat).unwrap();
    let lost = before - driver.state().player(seat(1)).unwrap().life();
    let dealt = if positive { power.abs() } else { power.max(0) };
    assert_eq!(lost, dealt, "power {power}, as positive {positive}");
    replays(&scenario, &driver);
}

#[rstest]
fn r377_unlife_turns_damage_into_poison_at_zero_life(
    #[values(2, 3, 4)] seats: usize,
    #[values(-3, 0, 1, 5)] life: i32,
) {
    let scenario = Scenario::new(seats)
        .at(3, 1, Step::Main1)
        .life(0, life)
        .battlefield(0, [ready(card("Phyrexian Unlife").unwrap())])
        .battlefield(1, [ready(mountain())])
        .hand(1, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let me = driver.target_player(0);
    driver.cast_at("Bolt", &[me]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let player = driver.state().player(seat(0)).unwrap();
    let poison = player.counters_of(&CounterKind::Poison);
    if life <= 0 {
        assert_eq!((player.life(), poison), (life, 3), "infect at {life}");
    } else {
        assert_eq!((player.life(), poison), (life - 3, 0), "damage at {life}");
    }
    assert!(driver.state().is_alive(seat(0)));
    replays(&scenario, &driver);
}

/// The ventures `player` made.
fn ventures(driver: &Driver, player: usize) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::Ventured(v) if v.player == seat(player)))
        .count()
}

#[rstest]
fn r377_an_opponent_ventures_once_each_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] venturer: usize,
) {
    let scenario = Scenario::new(seats)
        .at(3, venturer, Step::Main1)
        .battlefield(0, [ready(card("Keen-Eared Sentry").unwrap())])
        .hand(
            venturer,
            [w11d_venturer("Venture A"), w11d_venturer("Venture B")],
        );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    for name in ["Venture A", "Venture B"] {
        driver.cast(name).unwrap();
        while !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty() {
            driver.answer_passively().unwrap();
        }
    }
    let expected = if venturer == 0 { 2 } else { 1 };
    assert_eq!(ventures(&driver, venturer), expected, "seat {venturer}");
    replays(&scenario, &driver);
}
