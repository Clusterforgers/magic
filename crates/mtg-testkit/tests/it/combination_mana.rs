//! Goal: mana "in any combination of colors" read from Oracle text plays as the rules say at tables of two, three and four seats: an artifact that adds several mana pays a cost of any colors with one tap (paying its own cost of mana or energy first), a combination of listed types makes only those types, an Aura's bonus is a combination for the enchanted land's controller, and whole bot games with these cards keep every invariant and pick combinations.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::common::seeds::fuzz_games;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Normal;
use mtg_engine::mana::ManaOption;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, attached_to, play_game_with, ready};
use rstest::rstest;

use ManaType::{Black, Blue, Green, Red, White};

const COLORS: [ManaType; 5] = [White, Blue, Black, Red, Green];

fn spell(cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Trick")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Every seat has a library to draw from.
fn libraries(mut scenario: Scenario, seats: usize) -> Scenario {
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    scenario
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

/// The types of the mana the last payment of the events spent: the spell's, after the costs of the sources.
fn spent(events: &[Event]) -> BTreeSet<ManaType> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ManaSpent { mana, .. } => Some(mana.clone()),
            _ => None,
        })
        .next_back()
        .unwrap_or_default()
        .into_iter()
        .collect()
}

#[rstest]
#[case::five_colors_from_five_lands_and_the_cataracts(5, "{W}{U}{B}{R}{G}", Some(COLORS.to_vec()))]
#[case::one_land_short(4, "{W}{U}{B}{R}{G}", None)]
#[case::one_color_five_times(5, "{R}{R}{R}{R}{R}", Some(vec![Red]))]
fn an_artifact_that_costs_mana_adds_a_combination_that_pays_for_a_spell(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: usize,
    #[case] cost: &str,
    #[case] spent_types: Option<Vec<ManaType>>,
) {
    // The {5} of the ability is paid with the lands, and the five mana it adds pay the spell.
    let mut board = vec![ready(card("Cascading Cataracts").unwrap())];
    board.extend((0..lands).map(|_| ready(plains())));
    let scenario = Scenario::new(seats)
        .hand(0, [spell(cost)])
        .battlefield(0, board);
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    assert_eq!(
        castable(&driver),
        spent_types.is_some(),
        "{cost} with {lands} lands"
    );
    if let Some(types) = spent_types {
        let events = driver.cast("Trick").unwrap();
        let taps = events
            .iter()
            .filter(|event| matches!(event, Event::PermanentTapped { .. }))
            .count();
        assert_eq!(taps, lands + 1, "every land and the artifact: {events:?}");
        assert_eq!(spent(&events), types.into_iter().collect(), "{events:?}");
    }
}

#[rstest]
#[case::three_colors("{W}{U}{B}", true)]
#[case::three_of_one("{G}{G}{G}", true)]
#[case::four_mana("{W}{U}{B}{R}", false)]
fn conversion_apparatus_pays_three_energy_for_three_mana_of_any_colors(
    #[values(2, 3, 4)] seats: usize,
    #[case] cost: &str,
    #[case] affordable: bool,
    #[values(2, 3)] energy: u32,
) {
    let scenario = Scenario::new(seats)
        .hand(0, [spell(cost)])
        .battlefield(0, [ready(card("Conversion Apparatus").unwrap())])
        .counters(
            0,
            mtg_cards::value::CounterKind::Other("energy".into()),
            energy,
        );
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    // The energy is paid in the payment itself: three are needed.
    assert_eq!(
        castable(&driver),
        affordable && energy >= 3,
        "{cost}, {energy} energy"
    );
}

#[test]
fn a_combination_of_listed_types_makes_only_those_types() {
    let scenario = Scenario::new(2)
        .hand(0, [spell("{U}{U}"), spell("{W}{W}")])
        .battlefield(0, [ready(card("Relic of Sauron").unwrap())]);
    let driver = Driver::from_scenario(libraries(scenario, 2)).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority");
    };
    let made: BTreeSet<Vec<ManaType>> = options
        .mana_abilities
        .iter()
        .map(ManaOption::made)
        .collect();
    assert_eq!(
        made,
        BTreeSet::from([
            vec![Blue, Blue],
            vec![Blue, Black],
            vec![Blue, Red],
            vec![Black, Black],
            vec![Black, Red],
            vec![Red, Red],
        ])
    );
    assert_eq!(options.castable.len(), 1, "{{U}}{{U}} but not {{W}}{{W}}");
}

#[rstest]
#[case::the_aura_of_the_player("Dawn's Reflection", 0)]
#[case::the_aura_of_a_foe("Dawn's Reflection", 1)]
fn an_aura_adds_two_mana_in_any_combination_to_each_tap_of_the_land(
    #[values(2, 3, 4)] seats: usize,
    #[case] aura: &str,
    #[case] aura_seat: usize,
) {
    let enchantment = attached_to(ready(card(aura).unwrap()), 0, 0);
    let scenario = if aura_seat == 0 {
        Scenario::new(seats).battlefield(0, [ready(mountain()), enchantment])
    } else {
        Scenario::new(seats)
            .battlefield(0, [ready(mountain())])
            .battlefield(aura_seat, [enchantment])
    };
    let mut driver =
        Driver::from_scenario(libraries(scenario.hand(0, [spell("{R}{W}{U}")]), seats)).unwrap();
    assert!(castable(&driver), "R from the land, then W and U");
    let events = driver.cast("Trick").unwrap();
    let taps = events
        .iter()
        .filter(|event| matches!(event, Event::PermanentTapped { .. }))
        .count();
    assert_eq!(taps, 1, "one tap pays for the three mana: {events:?}");
    assert_eq!(spent(&events), BTreeSet::from([Red, White, Blue]));
}

/// Lands, the mana permanents of combinations and spells of every color to spend the mana on.
fn deck() -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = Vec::new();
    for _ in 0..8 {
        deck.extend([forest(), island(), mountain(), plains(), swamp()]);
    }
    for name in [
        "Cascading Cataracts",
        "Baxter Building",
        "Conversion Apparatus",
        "Dawn's Reflection",
        "Dawn's Reflection",
        "Relic of Sauron",
    ] {
        deck.push(card(name).unwrap());
    }
    for copy in 0..6 {
        deck.extend([
            spell_named(&format!("Rainbow {copy}"), "{W}{U}{B}{R}{G}"),
            spell_named(&format!("Pair {copy}"), "{B}{R}"),
            spell_named(&format!("Blue {copy}"), "{3}{U}"),
        ]);
    }
    deck
}

fn spell_named(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    /// Taps of one source that added two or more mana of different types.
    mixed: u64,
    /// Spells paid by a tap of a source that adds several mana (a mana of three or more in one batch).
    wide: u64,
}

fn tally(events: &[Event], seen: &mut Seen) {
    let mut batch: Vec<(mtg_core::ids::ObjectId, ManaType)> = Vec::new();
    let flush = |batch: &mut Vec<(mtg_core::ids::ObjectId, ManaType)>, seen: &mut Seen| {
        seen.mixed += u64::from(batch.iter().any(|(_, mana)| *mana != batch[0].1));
        seen.wide += u64::from(batch.len() >= 3);
        batch.clear();
    };
    for event in events {
        match event {
            Event::ManaAdded {
                mana,
                source: Some(source),
                only,
                ..
            } if only.is_empty() => {
                if batch.first().is_some_and(|(first, _)| first != source) {
                    flush(&mut batch, seen);
                }
                batch.push((*source, *mana));
            }
            _ => flush(&mut batch, seen),
        }
    }
}

#[rstest]
#[case::two(2)]
#[case::three(3)]
#[case::four(4)]
fn bot_games_with_these_cards_finish_and_keep_every_invariant(#[case] players: usize) {
    let seen = Mutex::new(Seen::default());
    let games: u64 = fuzz_games(10);
    for_each_seed(0..games, |seed| {
        let decks = (0..players).map(|_| deck()).collect();
        let setup = GameSetup::new(Arc::new(Normal), seed, decks);
        let options = PlayOptions {
            partial_payments: seed % 2 == 0,
            ..PlayOptions::default()
        };
        let outcome =
            play_game_with(setup, seed, options).unwrap_or_else(|failure| panic!("{failure}"));
        tally(&outcome.events, &mut seen.lock().unwrap());
    });
    let seen = seen.into_inner().unwrap();
    assert!(
        seen.mixed > 0 && seen.wide > 0,
        "the games should tap for a combination of three or more mana of two types: {} mixed, {} wide",
        seen.mixed,
        seen.wide
    );
}

#[test]
fn a_seeded_game_with_these_cards_is_the_same_game_every_time() {
    let play = |seed: u64| {
        let setup = GameSetup::new(Arc::new(Normal), seed, vec![deck(), deck()]);
        play_game_with(setup, seed, PlayOptions::default())
            .unwrap_or_else(|failure| panic!("{failure}"))
    };
    let (first, second) = (play(5), play(5));
    assert_eq!(first.final_hash, second.final_hash);
    assert_eq!(first.events, second.events);
}
