//! Goal: the cards that take control of groups or of a chosen permanent play as printed at tables of two, three and four seats: Zealous Conscripts takes, untaps and hastes a permanent for the turn only, The Nipton Lottery takes one random creature and destroys the rest, Unnatural Growth doubles the power and toughness of each creature its controller has at the beginning of combat and not an opponent's, Opportunistic Dragon silences and holds an opponent's Human until it leaves, Subjugate the Hobbits takes small noncommander creatures for good, and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::c4_control::{controller, replays, settle};
use crate::common::pid;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::PresetPermanent;
use mtg_engine::characteristics::controller_of;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, island, mountain, swamp};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

fn creatures_of(driver: &Driver, seat: usize) -> Vec<String> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id) == Some(pid(seat)))
        .filter_map(|id| state.object(*id))
        .filter(|object| {
            mtg_engine::characteristics(state, object.id())
                .is_some_and(|chars| chars.types.contains(CardType::Creature))
        })
        .map(|object| object.card().name.clone())
        .collect()
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = mtg_engine::characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn has_haste(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    keywords_of(driver.state(), id)
        .iter()
        .any(|keyword| keyword.name() == "Haste")
}

fn nine_lands() -> Vec<PresetPermanent> {
    let mut lands: Vec<_> = (0..4).map(|_| ready(island())).collect();
    lands.extend((0..3).map(|_| ready(mountain())));
    lands.extend((0..2).map(|_| ready(swamp())));
    lands
}

/// Seat 0 holds the corpus card and nine lands and has a Bear; seat 1 has a tapped "Ogre 1"; every other seat an Ogre.
fn table(seats: usize, name: &str) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(name).unwrap(), burn("Bolt", 4)])
        .battlefield(
            0,
            nine_lands()
                .into_iter()
                .chain([ready(free_creature("Bear", 2, 2))]),
        )
        .battlefield(1, [tapped(free_creature("Ogre 1", 3, 3))]);
    for seat in 2..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Ogre {seat}"), 3, 3))]);
    }
    scenario
}

#[rstest]
fn zealous_conscripts_takes_untaps_and_hastes_a_permanent_for_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Zealous Conscripts"))?;
    driver.cast("Zealous Conscripts")?;
    settle(&mut driver, Some("Ogre 1"))?;
    assert_eq!(controller(&driver, "Ogre 1"), 0);
    let id = driver.id_of("Ogre 1")?;
    let object = driver.state().object(id).unwrap();
    assert!(!object.permanent().unwrap().tapped());
    assert!(has_haste(&driver, "Ogre 1"));
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(
        controller(&driver, "Ogre 1"),
        1,
        "it goes home with the turn"
    );
    assert!(!has_haste(&driver, "Ogre 1"));
    replays(&driver);
    Ok(())
}

#[rstest]
fn the_nipton_lottery_takes_one_random_creature_and_destroys_the_rest(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4, 5)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "The Nipton Lottery").seed(seed))?;
    driver.cast("The Nipton Lottery")?;
    settle(&mut driver, None)?;
    let mine = creatures_of(&driver, 0);
    assert_eq!(mine.len(), 1, "{mine:?}");
    for seat in 1..seats {
        assert!(creatures_of(&driver, seat).is_empty(), "seat {seat}");
    }
    let taken = driver.id_of(&mine[0])?;
    assert!(
        !driver
            .state()
            .object(taken)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert!(has_haste(&driver, &mine[0]));
    replays(&driver);
    Ok(())
}

#[rstest]
fn unnatural_growth_doubles_each_creature_you_control_at_the_beginning_of_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Mind Flayer").battlefield(
        0,
        [
            ready(card("Unnatural Growth").unwrap()),
            ready(free_creature("Bear", 2, 2)),
            ready(free_creature("Wisp", 3, 1)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::BeginCombat)?;
    settle(&mut driver, None)?;
    assert_eq!(stats(&driver, "Bear"), (4, 4));
    assert_eq!(stats(&driver, "Wisp"), (6, 2));
    assert_eq!(
        stats(&driver, "Ogre 1"),
        (3, 3),
        "an opponent's creature is not doubled"
    );
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(stats(&driver, "Bear"), (2, 2));
    replays(&driver);
    Ok(())
}

#[rstest]
fn opportunistic_dragon_holds_an_opponents_human_silenced_while_it_remains(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] dragon_dies: bool,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Opportunistic Dragon").battlefield_named(1, ["Master Thief"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Opportunistic Dragon")?;
    settle(&mut driver, Some("Master Thief"))?;
    assert_eq!(controller(&driver, "Master Thief"), 0);
    let thief = driver.id_of("Master Thief")?;
    assert_eq!(
        mtg_engine::ability_reader::abilities_of(driver.state(), thief).len(),
        0
    );
    if dragon_dies {
        let dragon = driver.target_named("Opportunistic Dragon")?;
        driver.cast_at("Bolt", &[dragon])?;
        settle(&mut driver, None)?;
        assert_eq!(controller(&driver, "Master Thief"), 1);
        assert_ne!(
            mtg_engine::ability_reader::abilities_of(driver.state(), thief).len(),
            0
        );
    }
    replays(&driver);
    Ok(())
}

#[rstest]
fn subjugate_the_hobbits_takes_the_small_noncommander_creatures_for_good(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .format(std::sync::Arc::new(mtg_engine::format::Commander))
        .at(1, 0, Step::Main1)
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(mountain()),
                ready(card("Mind Flayer").unwrap()),
                ready(free_creature("Small", 1, 1)),
            ],
        )
        .hand(1, [card("Subjugate the Hobbits").unwrap()])
        .battlefield(1, (0..7).map(|_| ready(island())));
    for seat in 2..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Ogre {seat}"), 3, 3))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cmd")?;
    settle(&mut driver, None)?;
    driver.idle_until(2, Step::Main1)?;
    driver.cast("Subjugate the Hobbits")?;
    settle(&mut driver, None)?;
    assert_eq!(controller(&driver, "Small"), 1);
    assert_eq!(controller(&driver, "Cmd"), 0, "a commander stays");
    assert_eq!(controller(&driver, "Mind Flayer"), 0, "mana value 5 stays");
    for seat in 2..seats {
        assert_eq!(controller(&driver, &format!("Ogre {seat}")), 1);
    }
    replays(&driver);
    Ok(())
}
