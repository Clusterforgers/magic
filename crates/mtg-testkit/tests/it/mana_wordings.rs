//! Goal: the mana ability wordings read from Oracle text play as the rules say at tables of two, three and four seats: mana that may pay only for a creature of the type its source chose, an ability word before a mana ability with an activation condition (Ferocious), and a mana ability an Aura gives the enchanted land, whoever controls the Aura; whole bot games with these cards keep every invariant and use each mechanic.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::common::seeds::fuzz_games;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use common::seeds::for_each_seed;
use mtg_cards::CardDefinition;
use mtg_cards::spend::SpendOn;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::format::Normal;
use mtg_engine::mana::ManaOption;
use mtg_engine::object::Chosen;
use mtg_engine::{Event, GameSetup};
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, PlayOptions, Scenario, attached_to, play_game_with, ready};
use rstest::rstest;

fn creature(name: &str, subtype: &str, cost: &str, power: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(power, power)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

fn instant(cost: &str) -> Arc<CardDefinition> {
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

fn offered(driver: &Driver) -> Vec<ManaOption> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.mana_abilities.clone(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

fn castable(driver: &Driver) -> bool {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => !options.castable.is_empty(),
        other => panic!("priority was expected, got {other:?}"),
    }
}

#[rstest]
#[case::the_type_chosen(
    "Unclaimed Territory",
    "Elf",
    creature("Elf Scout", "Elf", "{R}", 1),
    true
)]
#[case::another_type(
    "Unclaimed Territory",
    "Elf",
    creature("Goblin Scout", "Goblin", "{R}", 1),
    false
)]
#[case::not_a_creature("Unclaimed Territory", "Elf", instant("{R}"), false)]
#[case::the_courtyard_pays_for_the_type(
    "Secluded Courtyard",
    "Goblin",
    creature("Goblin Scout", "Goblin", "{U}", 1),
    true
)]
#[case::the_courtyard_not_for_another(
    "Secluded Courtyard",
    "Goblin",
    creature("Elf Scout", "Elf", "{U}", 1),
    false
)]
fn mana_of_a_land_that_chose_a_type_pays_only_for_a_creature_of_that_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] land: &str,
    #[case] chosen: &str,
    #[case] spell: Arc<CardDefinition>,
    #[case] affordable: bool,
) {
    // The types to choose from are those of the creature cards in the game.
    let scenario = Scenario::new(seats)
        .hand(0, [card(land).unwrap(), spell])
        .hand(
            1,
            [
                creature("Elf Scout", "Elf", "{R}", 1),
                creature("Goblin Scout", "Goblin", "{R}", 1),
            ],
        );
    let mut driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    driver.play_land(land).unwrap();
    let DecisionKind::ChooseOption(options) = driver.decision().unwrap().kind.clone() else {
        panic!("a creature type is chosen");
    };
    let at = options
        .options
        .iter()
        .position(|option| matches!(option, ChoiceOption::Label(label) if label == chosen))
        .unwrap();
    driver
        .answer(Answer::ChooseOption(OptionChoice(at)))
        .unwrap();
    assert_eq!(castable(&driver), affordable);
    let colored = offered(&driver)
        .into_iter()
        .filter(|option| !option.only.is_empty())
        .count();
    assert_eq!(colored, 5, "one restricted way for each color: {land}");
}

#[test]
fn the_restriction_holds_the_type_that_was_chosen_and_the_courtyard_adds_the_ability_use() {
    let scenario = Scenario::new(2).hand(
        0,
        [
            card("Secluded Courtyard").unwrap(),
            creature("Elf Scout", "Elf", "{R}", 1),
            creature("Goblin Scout", "Goblin", "{R}", 1),
        ],
    );
    let mut driver = Driver::from_scenario(libraries(scenario, 2)).unwrap();
    driver.play_land("Secluded Courtyard").unwrap();
    let DecisionKind::ChooseOption(options) = driver.decision().unwrap().kind.clone() else {
        panic!("a creature type is chosen");
    };
    let at = options
        .options
        .iter()
        .position(|option| matches!(option, ChoiceOption::Label(label) if label == "Elf"))
        .unwrap();
    driver
        .answer(Answer::ChooseOption(OptionChoice(at)))
        .unwrap();
    let events = driver
        .tap_for_mana_of("Secluded Courtyard", ManaType::Red)
        .unwrap();
    let only: Vec<&Vec<SpendOn>> = events
        .iter()
        .filter_map(|event| match event {
            Event::ManaAdded { only, .. } => Some(only),
            _ => None,
        })
        .collect();
    assert_eq!(only.len(), 1);
    assert_eq!(
        only[0].len(),
        2,
        "a spell and an ability of the type: {only:?}"
    );
    for on in only[0] {
        let (SpendOn::Spell(filter) | SpendOn::Ability(filter)) = on else {
            panic!("{on:?}");
        };
        assert_eq!(filter.subtypes.len(), 1);
        assert_eq!(filter.subtypes[0].0, "Elf");
        assert!(!filter.chosen_type);
    }
}

/// Seat 0 controls Whisperer of the Wilds and a creature of power `power`; the offers of the Whisperer.
fn ferocious_offers(seats: usize, power: i32) -> Vec<Vec<ManaType>> {
    let big = creature("Brute", "Beast", "{G}", power);
    let scenario = Scenario::new(seats).battlefield(
        0,
        [ready(card("Whisperer of the Wilds").unwrap()), ready(big)],
    );
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    offered(&driver).iter().map(ManaOption::made).collect()
}

#[rstest]
#[case::power_four(4, true)]
#[case::power_five(5, true)]
#[case::power_three(3, false)]
#[case::power_one(1, false)]
fn the_ferocious_ability_is_offered_only_with_a_creature_of_power_four_or_more(
    #[values(2, 3, 4)] seats: usize,
    #[case] power: i32,
    #[case] doubled: bool,
) {
    let offers = ferocious_offers(seats, power);
    // The Whisperer itself is a 0/1 or so; the plain ability is always offered.
    assert!(offers.contains(&vec![ManaType::Green]), "{offers:?}");
    assert_eq!(
        offers.contains(&vec![ManaType::Green, ManaType::Green]),
        doubled,
        "{offers:?}"
    );
}

#[rstest]
#[case::any_color("Abundant Growth", 1)]
#[case::two_of_one_color("Gift of Paradise", 2)]
#[case::weirding_wood("Weirding Wood", 2)]
#[case::new_horizons("New Horizons", 2)]
fn an_aura_gives_the_enchanted_land_a_mana_ability_of_any_color_whoever_controls_the_aura(
    #[values(2, 3, 4)] seats: usize,
    #[case] aura: &str,
    #[case] count: usize,
    #[values(0, 1)] aura_seat: usize,
) {
    // The Aura of seat `aura_seat` enchants the Mountain of seat 0; the land's controller taps it.
    let enchantment = attached_to(ready(card(aura).unwrap()), 0, 0);
    let scenario = if aura_seat == 0 {
        Scenario::new(seats).battlefield(0, [ready(mountain()), enchantment])
    } else {
        Scenario::new(seats)
            .battlefield(0, [ready(mountain())])
            .battlefield(aura_seat, [enchantment])
    };
    let driver = Driver::from_scenario(libraries(scenario, seats)).unwrap();
    let offers: BTreeSet<Vec<ManaType>> = offered(&driver).iter().map(ManaOption::made).collect();
    let colors = [
        ManaType::White,
        ManaType::Blue,
        ManaType::Black,
        ManaType::Red,
        ManaType::Green,
    ];
    for color in colors {
        assert!(
            offers.contains(&vec![color; count]),
            "{color:?}: {offers:?}"
        );
    }
    assert!(
        offers.contains(&vec![ManaType::Red]),
        "its own mana: {offers:?}"
    );
}

#[test]
fn an_enchanted_land_taps_once_for_the_mana_it_has_of_its_own_and_the_mana_the_aura_gives() {
    let scenario = Scenario::new(2).battlefield(
        0,
        [
            ready(mountain()),
            attached_to(ready(card("Abundant Growth").unwrap()), 0, 0),
        ],
    );
    let mut driver = Driver::from_scenario(libraries(scenario, 2)).unwrap();
    let events = driver.tap_for_mana_of("Mountain", ManaType::Blue).unwrap();
    assert!(
        events.iter().any(|event| matches!(
            event,
            Event::ManaAdded {
                mana: ManaType::Blue,
                ..
            }
        )),
        "{events:?}"
    );
    assert!(
        offered(&driver).is_empty(),
        "the land is tapped: no second use"
    );
}

fn lands_and_spells() -> Vec<Arc<CardDefinition>> {
    let mut deck: Vec<Arc<CardDefinition>> = Vec::new();
    for _ in 0..7 {
        deck.extend([forest(), island(), mountain(), plains(), swamp()]);
    }
    deck
}

/// A deck of the new cards beside enough lands and creatures to use them.
fn deck() -> Vec<Arc<CardDefinition>> {
    let mut deck = lands_and_spells();
    for name in [
        "Unclaimed Territory",
        "Unclaimed Territory",
        "Secluded Courtyard",
        "Pillar of Origins",
        "Whisperer of the Wilds",
        "Fanatic of Rhonas",
        "Abundant Growth",
        "Gift of Paradise",
        "New Horizons",
        "Weirding Wood",
    ] {
        deck.push(card(name).unwrap());
    }
    for copy in 0..5 {
        deck.extend([
            creature(&format!("Elf Scout {copy}"), "Elf", "{1}{G}", 4),
            creature(&format!("Goblin Scout {copy}"), "Goblin", "{1}{R}", 2),
            creature(&format!("Wizard Scout {copy}"), "Wizard", "{U}", 1),
        ]);
    }
    deck
}

/// What a campaign saw.
#[derive(Default)]
struct Seen {
    chosen_types: u64,
    restricted_to_a_type: u64,
    doubled: u64,
    quadrupled: u64,
    enchanted_taps_of_two: u64,
}

fn tally(events: &[Event], seen: &mut Seen) {
    for event in events {
        match event {
            Event::Entered {
                chosen: Some(Chosen::CreatureType(_)),
                ..
            } => seen.chosen_types += 1,
            Event::ManaAdded { only, .. }
                if only
                    .iter()
                    .any(|on| matches!(on, SpendOn::Spell(f) if !f.subtypes.is_empty())) =>
            {
                seen.restricted_to_a_type += 1;
            }
            _ => {}
        }
    }
    // A tap adds all its mana in one batch: a source making two or four mana of one type is a ferocious or a
    // granted ability (the other sources of the deck add one mana).
    let mut batch: Vec<(mtg_core::ids::ObjectId, ManaType)> = Vec::new();
    let mut flush = |batch: &mut Vec<(mtg_core::ids::ObjectId, ManaType)>, seen: &mut Seen| {
        if let Some((source, mana)) = batch.first().copied()
            && batch.iter().all(|entry| *entry == (source, mana))
        {
            match batch.len() {
                2 => {
                    seen.doubled += 1;
                    seen.enchanted_taps_of_two += u64::from(mana != ManaType::Green);
                }
                4 => seen.quadrupled += 1,
                _ => {}
            }
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
    let games: u64 = fuzz_games(8);
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
        seen.chosen_types > 0 && seen.restricted_to_a_type > 0,
        "the games should choose a type and add restricted mana: {} chosen, {} restricted",
        seen.chosen_types,
        seen.restricted_to_a_type
    );
    assert!(
        seen.doubled > 0 && seen.quadrupled > 0,
        "the ferocious abilities should be used: doubled {}, quadrupled {}",
        seen.doubled,
        seen.quadrupled
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
