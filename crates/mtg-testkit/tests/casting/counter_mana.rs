//! Goal: Mana Bloom enters with the X it was cast with, spends a charge counter for one mana of any color once each turn and returns to its owner's hand at its controller's upkeep when no counter is left; Ramos grows by a +1/+1 counter for each color of a spell its controller casts (none for a colorless spell or another player's) and spends five counters for ten mana once each turn; at tables of two, three and four seats, and whole bot games with them finish and keep every invariant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::{Arc, Mutex};

use common::pid;
use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::format::Normal;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{
    Driver, PlayOptions, Scenario, ScriptError, play_game_with, ready, with_counters,
};
use rstest::rstest;

const BLOOM: &str = "Mana Bloom";
const RAMOS: &str = "Ramos, Dragon Engine";

fn charge() -> CounterKind {
    CounterKind::Charge
}

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Two of each basic land, ready.
fn lands() -> Vec<mtg_engine::setup::PresetPermanent> {
    [forest, island, mountain, plains, swamp]
        .into_iter()
        .flat_map(|land| [ready(land()), ready(land())])
        .collect()
}

/// A scenario of `seats` seats, each with a library.
fn seats(seats: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| plains()))
    })
}

/// The counters of `kind` on the first permanent named `name` of the battlefield.
fn counters(driver: &Driver, name: &str, kind: &CounterKind) -> Option<u32> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == name)
        .and_then(|object| object.permanent())
        .map(|permanent| permanent.counters_of(kind))
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// Casts a spell whenever one is castable, lets it resolve, and counts the casts.
fn casts(driver: &mut Driver) -> usize {
    let mut casts = 0;
    while castable(driver) {
        driver.cast("Spell").unwrap();
        settle(driver).unwrap();
        casts += 1;
    }
    casts
}

#[rstest]
fn mana_bloom_enters_with_the_x_it_was_cast_with(
    #[values(0, 1, 2)] x: u32,
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let scenario = seats(players)
        .hand(0, [card(BLOOM).unwrap()])
        .battlefield(0, (0..3).map(|_| ready(forest())));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast(BLOOM)?;
    driver.expect_decision(0, "choose-number")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(counters(&driver, BLOOM, &charge()), Some(x));
    Ok(())
}

#[rstest]
#[case::a_counter_pays_one_mana("{R}", 2, &[], 1, 1)]
#[case::once_each_turn_with_a_land("{R}", 2, &["Mountain"], 2, 1)]
#[case::no_counter_no_mana("{R}", 0, &[], 0, 0)]
#[case::one_mana_not_two("{R}{R}", 2, &[], 0, 2)]
#[case::the_bloom_and_a_land("{R}{R}", 1, &["Mountain"], 1, 0)]
#[case::lands_pay_before_the_counter("{R}", 1, &["Mountain", "Mountain"], 2, 1)]
fn mana_bloom_spends_a_counter_once_each_turn(
    #[case] cost: &str,
    #[case] counters_on_it: u32,
    #[case] lands: &[&str],
    #[case] expected: usize,
    #[case] left: u32,
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let bloom = with_counters(ready(card(BLOOM).unwrap()), charge(), counters_on_it);
    let scenario = seats(players)
        .hand(0, [spell(cost), spell(cost)])
        .battlefield(
            0,
            std::iter::once(bloom).chain(lands.iter().map(|_| ready(mountain()))),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let cast = casts(&mut driver);
    assert_eq!(cast, expected);
    assert_eq!(counters(&driver, BLOOM, &charge()), Some(left));
    Ok(())
}

#[rstest]
#[case::no_counter_goes_home(0, true)]
#[case::a_counter_stays(1, false)]
fn mana_bloom_returns_to_hand_at_the_upkeep_when_it_has_no_counter(
    #[case] counters_on_it: u32,
    #[case] returns: bool,
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let bloom = with_counters(ready(card(BLOOM).unwrap()), charge(), counters_on_it);
    let mut driver = Driver::from_scenario(seats(players).battlefield(0, [bloom]))?;
    // Seat 0's next turn is after every other seat has had one.
    driver.idle_until(1 + u32::try_from(players).unwrap(), Step::Upkeep)?;
    settle(&mut driver)?;
    let in_hand = common::names(driver.state(), Zone::Hand(pid(0))).contains(&BLOOM.to_owned());
    assert_eq!(in_hand, returns);
    assert_eq!(counters(&driver, BLOOM, &charge()).is_some(), !returns);
    Ok(())
}

#[rstest]
#[case::one_color("{U}", 1)]
#[case::two_colors("{W}{U}", 2)]
#[case::three_colors("{W}{U}{B}", 3)]
#[case::five_colors("{W}{U}{B}{R}{G}", 5)]
#[case::one_color_twice("{R}{R}", 1)]
#[case::one_color_and_generic("{2}{G}", 1)]
#[case::colorless("{2}", 0)]
fn ramos_grows_by_a_counter_for_each_color_of_a_spell_its_controller_casts(
    #[case] cost: &str,
    #[case] grows_by: u32,
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let scenario = seats(players).hand(0, [spell(cost)]).battlefield(
        0,
        std::iter::once(ready(card(RAMOS).unwrap())).chain(lands()),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Spell")?;
    settle(&mut driver)?;
    assert_eq!(
        counters(&driver, RAMOS, &CounterKind::PlusOnePlusOne),
        Some(grows_by)
    );
    Ok(())
}

#[rstest]
fn ramos_ignores_the_spells_of_other_players(
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let scenario = seats(players)
        .hand(1, [spell("{W}{U}")])
        .battlefield(0, [ready(card(RAMOS).unwrap())])
        .battlefield(1, lands())
        .at(1, 1, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Spell")?;
    settle(&mut driver)?;
    assert_eq!(
        counters(&driver, RAMOS, &CounterKind::PlusOnePlusOne),
        Some(0)
    );
    Ok(())
}

const RAINBOW: &str = "{W}{W}{U}{U}{B}{B}{R}{R}{G}{G}";

// A spell of five colors earns the five counters back once it is cast.
#[rstest]
#[case::five_counters_make_ten_mana("{10}", 5, 1, 0)]
#[case::four_are_not_enough("{10}", 4, 0, 4)]
#[case::once_each_turn("{10}", 10, 1, 5)]
#[case::the_mana_is_of_five_colors(RAINBOW, 5, 1, 5)]
#[case::four_are_not_enough_for_colors(RAINBOW, 4, 0, 4)]
fn ramos_spends_five_counters_for_ten_mana_once_each_turn(
    #[case] cost: &str,
    #[case] counters_on_it: u32,
    #[case] expected: usize,
    #[case] left: u32,
    #[values(2, 3, 4)] players: usize,
) -> Result<(), ScriptError> {
    let ramos = with_counters(
        ready(card(RAMOS).unwrap()),
        CounterKind::PlusOnePlusOne,
        counters_on_it,
    );
    let scenario = seats(players)
        .hand(0, [spell(cost), spell(cost)])
        .battlefield(0, [ramos]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(casts(&mut driver), expected);
    assert_eq!(
        counters(&driver, RAMOS, &CounterKind::PlusOnePlusOne),
        Some(left)
    );
    Ok(())
}

fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 12),
        ("Island", 5),
        ("Mountain", 5),
        ("Plains", 5),
        ("Swamp", 5),
        ("Mana Bloom", 4),
        ("Ramos, Dragon Engine", 3),
        ("Llanowar Elves", 2),
        ("Grizzly Bears", 4),
        ("Serra Angel", 3),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
        ("Counterspell", 3),
        ("Doom Blade", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| {
        (0..copies).map(move |_| match name {
            "Forest" => forest(),
            "Island" => island(),
            "Mountain" => mountain(),
            "Plains" => plains(),
            "Swamp" => swamp(),
            other => card(other).unwrap(),
        })
    })
    .collect()
}

/// The counters spent on mana and the counters Ramos earns in a campaign.
fn counter_traffic(events: &[Event]) -> (u64, u64) {
    let (mut spent, mut earned) = (0, 0);
    for event in events {
        if let Event::CountersChanged(changed) = event {
            if changed.delta < 0 && changed.counter == charge() {
                spent += 1;
            }
            if changed.delta > 0 && changed.counter == CounterKind::PlusOnePlusOne {
                earned += 1;
            }
        }
    }
    (spent, earned)
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_these_permanents_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new((0u64, 0u64));
    let games: u64 = std::env::var("MTG_FUZZ_GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    for_each_seed(0..games, |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        let (spent, earned) = counter_traffic(&outcome.events);
        let mut seen = seen.lock().unwrap();
        seen.0 += spent;
        seen.1 += earned;
    });
    let (spent, earned) = seen.into_inner().unwrap();
    assert!(spent > 0, "the games should spend charge counters");
    assert!(earned > 0, "the games should grow a Ramos");
}
