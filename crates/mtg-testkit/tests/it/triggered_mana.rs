//! Goal: the triggered mana abilities read from Oracle text (CR 605.1b) play as the rules say at tables of two, three and four seats: the mana of "Whenever you tap a land for mana, add one mana of any type that land produced" and "Whenever enchanted land is tapped for mana, its controller adds an additional {G}" joins the tap that triggers it for the player who taps, whoever controls the Aura or the enchantment, pays for spells in the same tap, and never uses the stack; whole bot games with these cards keep every invariant and use each mechanic.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::common::seeds::fuzz_games;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
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

fn spell(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
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

fn priority(driver: &Driver) -> mtg_engine::decision::PriorityOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => (**options).clone(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

fn offers(driver: &Driver) -> BTreeSet<Vec<ManaType>> {
    priority(driver)
        .mana_abilities
        .iter()
        .map(ManaOption::made)
        .collect()
}

fn castable(driver: &Driver) -> bool {
    !priority(driver).castable.is_empty()
}

#[rstest]
#[case::wild_growth("Wild Growth", vec![vec![Red, Green]])]
#[case::overgrowth("Overgrowth", vec![vec![Red, Green, Green]])]
#[case::fertile_ground("Fertile Ground", COLORS.iter().map(|color| vec![Red, *color]).collect())]
fn an_aura_adds_its_mana_to_each_tap_of_the_land_it_enchants_whoever_controls_the_aura(
    #[values(2, 3, 4)] seats: usize,
    #[case] aura: &str,
    #[case] expected: Vec<Vec<ManaType>>,
    #[values(0, 1)] aura_seat: usize,
) {
    // The Aura of seat `aura_seat` enchants the Mountain of seat 0, who taps it.
    let enchantment = attached_to(ready(card(aura).unwrap()), 0, 0);
    let scenario = if aura_seat == 0 {
        Scenario::new(seats).battlefield(0, [ready(mountain()), enchantment])
    } else {
        Scenario::new(seats)
            .battlefield(0, [ready(mountain())])
            .battlefield(aura_seat, [enchantment])
    };
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    assert_eq!(offers(&driver), expected.into_iter().collect());
}

#[rstest]
#[case::wild_growth("Wild Growth", "{R}{G}", true)]
#[case::wild_growth_wants_more("Wild Growth", "{R}{G}{G}", false)]
#[case::overgrowth("Overgrowth", "{R}{G}{G}", true)]
#[case::fertile_ground_picks_the_color("Fertile Ground", "{R}{U}", true)]
#[case::fertile_ground_adds_one_mana("Fertile Ground", "{R}{U}{U}", false)]
fn the_mana_of_a_triggered_ability_pays_for_a_spell_in_the_tap_of_one_land(
    #[values(2, 3, 4)] seats: usize,
    #[case] aura: &str,
    #[case] cost: &str,
    #[case] affordable: bool,
) {
    let scenario = Scenario::new(seats)
        .hand(0, [spell("Trick", cost)])
        .battlefield(
            0,
            [
                ready(mountain()),
                attached_to(ready(card(aura).unwrap()), 0, 0),
            ],
        );
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    assert_eq!(castable(&driver), affordable, "{aura}: {cost}");
    if affordable {
        let events = driver.cast("Trick").unwrap();
        let taps = events
            .iter()
            .filter(|event| matches!(event, Event::PermanentTapped { .. }))
            .count();
        assert_eq!(taps, 1, "one tap pays: {events:?}");
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::AbilityPutOnStack { .. })),
            "a mana ability uses no stack: {events:?}"
        );
    }
}

#[rstest]
#[case::mirari("Mirari's Wake")]
#[case::zendikar_resurgent("Zendikar Resurgent")]
fn a_permanent_that_says_you_tap_a_land_doubles_only_your_taps(
    #[values(2, 3, 4)] seats: usize,
    #[case] permanent: &str,
    #[values(0, 1)] owner: usize,
) {
    // Seat 0 taps an Island; the permanent is seat 0's or seat 1's.
    let mut scenario = Scenario::new(seats).hand(0, [spell("Trick", "{U}{U}")]);
    scenario = if owner == 0 {
        scenario.battlefield(0, [ready(island()), ready(card(permanent).unwrap())])
    } else {
        scenario
            .battlefield(0, [ready(island())])
            .battlefield(owner, [ready(card(permanent).unwrap())])
    };
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    assert_eq!(castable(&driver), owner == 0, "{permanent} of seat {owner}");
    let expected = if owner == 0 {
        vec![Blue, Blue]
    } else {
        vec![Blue]
    };
    assert_eq!(offers(&driver), BTreeSet::from([expected]));
}

#[rstest]
#[case::two_triggers_of_any_type(vec!["Mirari's Wake", "Zendikar Resurgent"], Green, vec![Green; 3])]
#[case::a_swamp_trigger_on_a_forest(vec!["Crypt Ghast"], Green, vec![Green])]
#[case::a_swamp_trigger_on_a_swamp(vec!["Crypt Ghast"], Black, vec![Black; 2])]
#[case::two_swamp_triggers(vec!["Crypt Ghast", "Nirkana Revenant"], Black, vec![Black; 3])]
#[case::mirari_and_ghast_on_a_swamp(vec!["Mirari's Wake", "Crypt Ghast"], Black, vec![Black; 3])]
fn triggers_add_their_mana_one_after_the_other_to_the_land_they_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] permanents: Vec<&str>,
    #[case] tapped: ManaType,
    #[case] expected: Vec<ManaType>,
) {
    let land = if tapped == Green { forest() } else { swamp() };
    let mut board = vec![ready(land)];
    board.extend(
        permanents
            .into_iter()
            .map(|name| ready(card(name).unwrap())),
    );
    let scenario = Scenario::new(seats).battlefield(0, board);
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    assert_eq!(offers(&driver), BTreeSet::from([expected]));
}

#[test]
fn tapping_at_priority_adds_the_extra_mana_in_the_same_batch_with_nothing_on_the_stack() {
    let scenario = Scenario::new(2).battlefield(
        0,
        [
            ready(forest()),
            ready(card("Mirari's Wake").unwrap()),
            ready(card("Zendikar Resurgent").unwrap()),
        ],
    );
    let mut driver = Driver::from_scenario(libraries(scenario, 2)).unwrap();
    let events = driver.tap_for_mana_of("Forest", Green).unwrap();
    let added: Vec<ManaType> = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { mana, .. } => Some(*mana),
            _ => None,
        })
        .collect();
    assert_eq!(added, [Green; 3], "{events:?}");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::AbilityPutOnStack { .. })),
        "{events:?}"
    );
    // Nothing waits for an answer but priority.
    assert!(matches!(
        driver.decision().unwrap().kind,
        DecisionKind::Priority(_)
    ));
}

/// Lands, the triggered mana permanents and spells to spend the mana on.
fn deck() -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = Vec::new();
    for _ in 0..7 {
        deck.extend([forest(), island(), mountain(), plains(), swamp()]);
    }
    for name in [
        "Wild Growth",
        "Wild Growth",
        "Overgrowth",
        "Fertile Ground",
        "Fertile Ground",
        "Mirari's Wake",
        "Zendikar Resurgent",
        "Crypt Ghast",
        "Nirkana Revenant",
        "Utopia Sprawl",
    ] {
        deck.push(card(name).unwrap());
    }
    for copy in 0..6 {
        deck.extend([
            spell(&format!("Big Trick {copy}"), "{3}{G}{G}"),
            spell(&format!("Mid Trick {copy}"), "{2}{U}"),
            spell(&format!("Small Trick {copy}"), "{R}"),
        ]);
    }
    deck
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    /// Taps of a land that added more than one mana, all in one batch with the tap.
    bonus: u64,
    /// Taps that added mana of two types.
    mixed: u64,
}

fn tally(events: &[Event], seen: &mut Seen) {
    let mut batch: Vec<(ObjectId, ManaType)> = Vec::new();
    let flush = |batch: &mut Vec<(ObjectId, ManaType)>, seen: &mut Seen| {
        if batch.len() >= 2 {
            seen.bonus += 1;
            seen.mixed += u64::from(batch.iter().any(|(_, mana)| *mana != batch[0].1));
        }
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
        seen.bonus > 0 && seen.mixed > 0,
        "the games should tap lands for more than one mana, of two types: {} taps, {} mixed",
        seen.bonus,
        seen.mixed
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
