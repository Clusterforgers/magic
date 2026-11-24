//! Goal: Snowfall's optional extra {U} ({U}{U} from a snow Island) joins an Island's tap only as mana for cumulative
//! upkeep, so one Island pays an upkeep of two (three when snow) but never a spell; Benthic Explorers untaps a tapped
//! land an opponent controls to add a mana of a type that land could produce, and has no mana without one; at two to
//! four seats, and each game replays to its live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ready, tapped};
use rstest::rstest;

/// An Island, snow when `snow` is set.
fn an_island(snow: bool) -> Arc<CardDefinition> {
    let mut land = CardDefinition::basic_land(Some(Color::Blue));
    "Island".clone_into(&mut land.name);
    if snow {
        land = land.with_supertype(Supertype::Snow);
    }
    Arc::new(land)
}

/// An instant named `name` costing `cost`.
fn instant(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// The card named `name` in `zone`.
fn in_zone(driver: &Driver, zone: Zone, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|object| object.card().name == name)
        })
        .unwrap()
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    })
}

#[rstest]
#[case::plain_island_pays_two(false, 1, true)]
#[case::plain_island_misses_three(false, 2, false)]
#[case::snow_island_pays_three(true, 2, true)]
#[case::snow_island_misses_four(true, 3, false)]
fn snowfall_adds_upkeep_mana_to_an_island(
    #[values(2, 3, 4)] seats: usize,
    #[case] snow: bool,
    #[case] age: u32,
    #[case] kept: bool,
) {
    let snowfall = with_counters(
        ready(card("Snowfall").unwrap()),
        CounterKind::Other("age".to_owned()),
        age,
    );
    let scenario = Scenario::new(seats).battlefield(0, [snowfall, ready(an_island(snow))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)
        .unwrap();
    let mut paid = false;
    for _ in 0..60 {
        match &driver.decision().unwrap().kind {
            DecisionKind::YesNo(options) => {
                let yes = !options.only_no;
                driver.answer(Answer::YesNo(YesNoChoice(yes))).unwrap();
            }
            DecisionKind::PayMana(_) if !paid => {
                paid = true;
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    assert_eq!(on_battlefield(&driver, "Snowfall"), kept);
    replays(&driver);
}

#[rstest]
fn snowfall_mana_pays_no_spell(#[values(2, 3, 4)] seats: usize, #[values(false, true)] snow: bool) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [ready(card("Snowfall").unwrap()), ready(an_island(snow))],
        )
        .hand(
            0,
            [instant("Two Blue", "{U}{U}"), instant("One Blue", "{U}")],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    let island = driver.id_of("Island").unwrap();
    let ways: Vec<_> = options
        .mana_abilities
        .iter()
        .filter(|o| o.source.object == island)
        .collect();
    assert!(
        ways.iter().any(|o| o.r120_restricted.is_empty()),
        "the extra mana may be declined"
    );
    assert!(
        ways.iter().any(|o| o
            .r120_restricted
            .iter()
            .map(|p| p.mana.len())
            .sum::<usize>()
            == if snow { 2 } else { 1 }),
        "{ways:?}"
    );
    let castable: Vec<_> = options.castable.clone();
    let two = in_zone(&driver, Zone::Hand(PlayerId(0)), "Two Blue");
    let one = in_zone(&driver, Zone::Hand(PlayerId(0)), "One Blue");
    assert!(!castable.contains(&two));
    assert!(castable.contains(&one));
    driver.cast("One Blue").unwrap();
    replays(&driver);
}

#[rstest]
#[case::green_from_a_glade("Green Spell", "{G}", "Cinder Glade")]
#[case::red_from_a_glade("Red Spell", "{R}", "Cinder Glade")]
#[case::blue_from_an_island("Blue Spell", "{U}", "Island")]
fn benthic_explorers_untaps_an_opponents_land(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] cost: &str,
    #[case] land: &str,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("Benthic Explorers").unwrap())])
        .battlefield(
            1,
            [
                tapped(card("Cinder Glade").unwrap()),
                tapped(island()),
                ready(forest()),
            ],
        )
        .hand(0, [instant(spell, cost)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast(spell).unwrap();
    let explorers = driver.id_of("Benthic Explorers").unwrap();
    let untapped = driver.id_of(land).unwrap();
    let tapped_now = |id| {
        driver
            .state()
            .object(id)
            .and_then(|object| object.permanent())
            .is_some_and(mtg_engine::object::PermanentState::tapped)
    };
    assert!(tapped_now(explorers));
    assert!(!tapped_now(untapped));
    assert_eq!(driver.state().stack().len(), 1);
    replays(&driver);
}

#[rstest]
fn benthic_explorers_needs_a_tapped_opponents_land(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [ready(card("Benthic Explorers").unwrap()), tapped(island())],
        )
        .battlefield(1, [ready(island())])
        .hand(0, [instant("Blue Spell", "{U}")]);
    let driver = Driver::from_scenario(scenario).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    assert!(
        options.castable.is_empty(),
        "no tapped land of an opponent, no mana"
    );
    assert_eq!(driver.state().player(PlayerId(0)).unwrap().life(), 20);
}
