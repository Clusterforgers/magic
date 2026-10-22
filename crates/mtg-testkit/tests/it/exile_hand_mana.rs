//! Goal: a mana ability whose cost exiles a card from the hand (Cadaverous Bloom: BB or GG) is a source of every payment only while the payer holds a card besides the spell being cast, the engine exiles a spell before a land and never the spell it pays for, one activation adds one list of mana, two such sources exile two cards, and whole bot games with these sources finish and keep every invariant at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::{Arc, Mutex};

use common::pid;
use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::events::ZoneCause;
use mtg_engine::format::Normal;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, ScriptError, play_game_with, ready};
use rstest::rstest;

fn definition(name: &str) -> Arc<CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Mountain" => mountain(),
        "Plains" => plains(),
        "Swamp" => swamp(),
        other => card(other).unwrap(),
    }
}

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Spell")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Seat 0 holds `spells` spells of `cost` and the cards `others`, and has `mine` in play; every seat has a library.
fn table(
    seats: usize,
    cost: &str,
    spells: usize,
    others: &[&str],
    mine: &[&str],
) -> Result<Driver, ScriptError> {
    let hand: Vec<Arc<CardDefinition>> = (0..spells)
        .map(|_| spell(cost))
        .chain(others.iter().map(|name| definition(name)))
        .collect();
    let mut scenario = Scenario::new(seats)
        .hand(0, hand)
        .battlefield(0, mine.iter().map(|name| ready(definition(name))));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..12).map(|_| plains()));
    }
    Driver::from_scenario(scenario)
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// Casts a spell whenever one is castable and counts the casts.
fn casts(driver: &mut Driver) -> usize {
    let mut casts = 0;
    while castable(driver) {
        driver.cast("Spell").unwrap();
        casts += 1;
    }
    casts
}

fn hand(driver: &Driver) -> Vec<String> {
    common::names(driver.state(), Zone::Hand(pid(0)))
}

fn exiled(driver: &Driver) -> Vec<String> {
    common::names(driver.state(), Zone::Exile)
}

/// A card that no mana of these tests can cast, so it is only ever exiled.
const FILLER: &str = "Serra Angel";
const BLOOM: &str = "Cadaverous Bloom";

#[rstest]
#[case::a_card_besides_the_spell("{B}{B}", 1, &[FILLER], &[BLOOM], 1)]
#[case::the_spell_is_not_there_to_exile("{B}{B}", 1, &[], &[BLOOM], 0)]
#[case::a_second_spell_is_the_card("{B}{B}", 2, &[], &[BLOOM], 1)]
#[case::green_from_the_other_list("{G}{G}", 1, &[FILLER], &[BLOOM], 1)]
#[case::one_list_for_one_activation("{B}{G}", 1, &[FILLER], &[BLOOM], 0)]
#[case::two_lists_need_two_sources("{B}{B}{G}{G}", 1, &[FILLER, FILLER], &[BLOOM], 0)]
#[case::no_red("{R}{R}", 1, &[FILLER], &[BLOOM], 0)]
#[case::three_black_with_a_swamp("{B}{B}{B}", 1, &[FILLER], &[BLOOM, "Swamp"], 1)]
#[case::three_black_without_one("{B}{B}{B}", 1, &[FILLER], &[BLOOM], 0)]
#[case::two_blooms_make_both_lists("{B}{B}{G}{G}", 1, &[FILLER, FILLER], &[BLOOM, BLOOM], 1)]
#[case::two_blooms_need_two_cards("{B}{B}{G}{G}", 1, &[FILLER], &[BLOOM, BLOOM], 0)]
#[case::the_bloom_beside_a_land("{B}", 2, &[FILLER], &[BLOOM, "Swamp"], 2)]
fn a_source_that_exiles_a_card_pays_only_with_a_card_to_exile(
    #[case] cost: &str,
    #[case] spells: usize,
    #[case] others: &[&str],
    #[case] mine: &[&str],
    #[case] expected: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, cost, spells, others, mine)?;
    assert_eq!(casts(&mut driver), expected);
    Ok(())
}

#[rstest]
fn the_card_exiled_is_a_spell_before_a_land_and_never_the_spell_cast(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "{B}{B}", 1, &["Swamp", FILLER], &[BLOOM])?;
    let events = driver.cast("Spell")?;
    assert_eq!(exiled(&driver), [FILLER], "the spell card, not the land");
    assert_eq!(hand(&driver), ["Swamp"]);
    let added = events
        .iter()
        .filter(|event| matches!(event, Event::ManaAdded { .. }))
        .count();
    assert_eq!(added, 2, "two black mana for one activation");
    Ok(())
}

#[rstest]
fn a_land_is_exiled_when_it_is_the_only_card(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "{G}{G}", 1, &["Swamp"], &[BLOOM])?;
    driver.cast("Spell")?;
    assert_eq!(exiled(&driver), ["Swamp"]);
    assert_eq!(hand(&driver), Vec::<String>::new());
    Ok(())
}

#[rstest]
fn two_blooms_exile_two_different_cards(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(
        seats,
        "{B}{B}{G}{G}",
        1,
        &[FILLER, "Doom Blade"],
        &[BLOOM, BLOOM],
    )?;
    let events = driver.cast("Spell")?;
    let mut gone = exiled(&driver);
    gone.sort();
    assert_eq!(gone, ["Doom Blade", FILLER]);
    let exiles = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    to: Zone::Exile,
                    cause: ZoneCause::Exiled,
                    ..
                }
            )
        })
        .count();
    assert_eq!(exiles, 2);
    Ok(())
}

#[rstest]
fn the_sources_name_the_cards_they_may_exile(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "{B}{B}", 1, &[FILLER, "Swamp"], &[BLOOM])?;
    driver.begin_cast("Spell")?;
    let options = driver.pay_options()?;
    assert_eq!(options.sources.len(), 2, "one source for each list");
    for source in &options.sources {
        let [take] = source.takes.as_slice() else {
            panic!("one part exiles a card: {:?}", source.takes);
        };
        assert!(take.exile && !take.sacrifice);
        assert_eq!(take.count, 1);
        assert_eq!(take.from.len(), 2, "the card and the land, not the spell");
        for id in &take.from {
            let object = driver.state().object(*id).unwrap();
            assert_eq!(object.zone(), Zone::Hand(pid(0)));
        }
    }
    Ok(())
}

fn deck() -> Vec<Arc<CardDefinition>> {
    [
        ("Forest", 11),
        ("Island", 4),
        ("Mountain", 5),
        ("Plains", 4),
        ("Swamp", 12),
        ("Cadaverous Bloom", 4),
        ("Llanowar Elves", 2),
        ("Grizzly Bears", 4),
        ("Serra Angel", 2),
        ("Giant Growth", 3),
        ("Lightning Bolt", 3),
        ("Counterspell", 3),
        ("Doom Blade", 3),
    ]
    .into_iter()
    .flat_map(|(name, copies)| (0..copies).map(move |_| definition(name)))
    .collect()
}

/// The cards exiled from a hand in a campaign, which only the cost of the Bloom does in these decks.
fn exiled_from_hands(events: &[Event]) -> u64 {
    events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    from: Zone::Hand(_),
                    to: Zone::Exile,
                    cause: ZoneCause::Exiled,
                    ..
                }
            )
        })
        .count() as u64
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_these_sources_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(0u64);
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
        *seen.lock().unwrap() += exiled_from_hands(&outcome.events);
    });
    assert!(
        seen.into_inner().unwrap() > 0,
        "the games should use these sources"
    );
}
