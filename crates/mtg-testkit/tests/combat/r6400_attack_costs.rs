//! Goal: costs to attack or block that take permanents (CR 508.1g, 509.1d) play as printed at two to four seats:
//! Leviathan attacks only by sacrificing two Islands (asked which when there are more, taken without a question when
//! there are exactly two, refused with fewer); Flooded Woodlands and Reclamation take a land for each creature of their
//! color that attacks and nothing for one of another color; Floodtide Serpent returns an enchantment its controller
//! controls; Hollow Warrior taps an untapped creature not being declared to attack or block; and every log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{creature, free_creature, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// Seat 0 is about to declare attackers with `mine`; seat 1 holds `theirs`.
fn table(
    seats: usize,
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, mine)
            .battlefield(1, theirs),
    )
}

/// How many permanents named `name` are on the battlefield.
fn count(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name)
        .count()
}

/// Whether the attack was declared.
fn declared(driver: &Driver) -> bool {
    driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::AttackersDeclared { .. }))
}

/// Answers a pick of the cost with the first objects it offers, as many as it takes; `None` when none is asked.
fn pick_first(driver: &mut Driver) -> Result<Option<usize>, ScriptError> {
    let DecisionKind::ChooseCards(options) = driver.decision()?.kind.clone() else {
        return Ok(None);
    };
    let taken: Vec<_> = options
        .from
        .iter()
        .copied()
        .take(options.min as usize)
        .collect();
    driver.answer(Answer::ChooseCards(CardsChoice(taken)))?;
    Ok(Some(options.from.len()))
}

/// The log replays to the same state.
fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::no_islands(0, false, None)]
#[case::one_island(1, false, None)]
#[case::two_islands(2, true, None)]
#[case::three_islands(3, true, Some(3))]
fn r6400_leviathan_sacrifices_two_islands_to_attack(
    #[values(2, 3, 4)] seats: usize,
    #[case] islands: usize,
    #[case] attacks: bool,
    #[case] asked: Option<usize>,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Leviathan").unwrap())];
    mine.extend(lands(islands, island));
    mine.extend(lands(2, plains));
    let mut driver = table(seats, mine, Vec::new())?;
    let made = driver.attack(&[("Leviathan", 1)]);
    assert_eq!(made.is_ok(), attacks, "{islands} Islands");
    if !attacks {
        assert!(!declared(&driver));
        assert_eq!(count(&driver, "Island"), islands, "nothing is sacrificed");
        return Ok(());
    }
    assert_eq!(pick_first(&mut driver)?, asked, "the Islands offered");
    assert!(declared(&driver));
    assert_eq!(count(&driver, "Island"), islands - 2);
    assert_eq!(count(&driver, "Plains"), 2, "only Islands pay");
    let graveyard = driver.state().zone(Zone::Graveyard(pid(0))).len();
    assert_eq!(graveyard, 2);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::woodlands_two_green_two_lands("Flooded Woodlands", Color::Green, 2, 2, 2, true)]
#[case::woodlands_two_green_one_land("Flooded Woodlands", Color::Green, 2, 0, 1, false)]
#[case::woodlands_one_green_one_other("Flooded Woodlands", Color::Green, 1, 1, 1, true)]
#[case::woodlands_other_only("Flooded Woodlands", Color::Green, 0, 2, 0, true)]
#[case::reclamation_three_black("Reclamation", Color::Black, 3, 0, 3, true)]
#[case::reclamation_two_black_one_land("Reclamation", Color::Black, 2, 0, 1, false)]
fn r6400_a_land_for_each_creature_of_the_color(
    #[values(2, 3, 4)] seats: usize,
    #[case] taxer: &str,
    #[case] color: Color,
    #[case] colored: usize,
    #[case] other: usize,
    #[case] land_count: usize,
    #[case] attacks: bool,
) -> Result<(), ScriptError> {
    let other_color = if color == Color::Green {
        Color::Red
    } else {
        Color::White
    };
    let mut mine: Vec<PresetPermanent> = (0..colored)
        .map(|n| ready(creature(&format!("Tinted {n}"), 0, &[color], 1, 1)))
        .chain((0..other).map(|n| ready(creature(&format!("Plain {n}"), 0, &[other_color], 1, 1))))
        .collect();
    mine.extend(lands(land_count, plains));
    // The enchantment taxes every player's creatures, whoever controls it.
    let mut driver = table(seats, mine, vec![ready(card(taxer).unwrap())])?;
    let attackers: Vec<String> = (0..colored)
        .map(|n| format!("Tinted {n}"))
        .chain((0..other).map(|n| format!("Plain {n}")))
        .collect();
    let named: Vec<(&str, usize)> = attackers.iter().map(|name| (name.as_str(), 1)).collect();
    let made = driver.attack(&named);
    assert_eq!(
        made.is_ok(),
        attacks,
        "{colored} of the color, {land_count} lands"
    );
    if attacks {
        while pick_first(&mut driver)?.is_some() {}
        assert!(declared(&driver));
        assert_eq!(count(&driver, "Plains"), land_count - colored);
        replays(&driver);
    } else {
        assert_eq!(count(&driver, "Plains"), land_count);
    }
    Ok(())
}

#[rstest]
#[case::no_enchantment(false)]
#[case::an_enchantment(true)]
fn r6400_floodtide_serpent_returns_an_enchantment(
    #[values(2, 3, 4)] seats: usize,
    #[case] enchantment: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Floodtide Serpent").unwrap())];
    if enchantment {
        mine.push(ready(card("Reclamation").unwrap()));
    }
    let mut driver = table(seats, mine, Vec::new())?;
    assert_eq!(
        driver.attack(&[("Floodtide Serpent", 1)]).is_ok(),
        enchantment
    );
    if enchantment {
        assert!(declared(&driver));
        assert_eq!(count(&driver, "Reclamation"), 0);
        let hand = driver.state().zone(Zone::Hand(pid(0)));
        assert!(
            hand.iter()
                .filter_map(|id| driver.state().object(*id))
                .any(|object| object.card().name == "Reclamation"),
            "the enchantment is returned to its owner's hand"
        );
        replays(&driver);
    }
    Ok(())
}

/// Whether the permanent named `name` is tapped.
fn tapped(driver: &Driver, name: &str) -> bool {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == name)
        .and_then(|object| {
            object
                .permanent()
                .map(mtg_engine::object::PermanentState::tapped)
        })
        .unwrap_or(false)
}

#[rstest]
#[case::alone(&["Hollow Warrior"], &[], false)]
#[case::with_a_helper(&["Hollow Warrior"], &["Helper"], true)]
#[case::helper_attacks_too(&["Hollow Warrior", "Helper"], &["Helper"], false)]
#[case::helper_and_a_spare(&["Hollow Warrior", "Helper"], &["Helper", "Spare"], true)]
fn r6400_hollow_warrior_taps_a_creature_to_attack(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[&str],
    #[case] others: &[&str],
    #[case] attacks: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(card("Hollow Warrior").unwrap())];
    mine.extend(others.iter().map(|name| ready(free_creature(name, 1, 1))));
    let mut driver = table(seats, mine, Vec::new())?;
    let named: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    assert_eq!(
        driver.attack(&named).is_ok(),
        attacks,
        "{attackers:?} with {others:?}"
    );
    if attacks {
        assert!(declared(&driver));
        let helper = others
            .iter()
            .find(|name| !attackers.contains(name))
            .unwrap();
        assert!(tapped(&driver, helper), "{helper} pays");
        replays(&driver);
    }
    Ok(())
}

#[rstest]
#[case::alone(&[], false)]
#[case::with_a_helper(&["Helper"], true)]
fn r6400_hollow_warrior_taps_a_creature_to_block(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: &[&str],
    #[case] blocks: bool,
) -> Result<(), ScriptError> {
    let mut theirs = vec![ready(card("Hollow Warrior").unwrap())];
    theirs.extend(others.iter().map(|name| ready(free_creature(name, 1, 1))));
    let mut driver = table(seats, vec![ready(free_creature("Raider", 2, 2))], theirs)?;
    driver.attack(&[("Raider", 1)])?;
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(
        driver.block(&[("Hollow Warrior", "Raider")]).is_ok(),
        blocks
    );
    if blocks {
        assert!(tapped(&driver, "Helper"), "the helper pays");
        assert!(
            !tapped(&driver, "Hollow Warrior"),
            "the blocker does not tap"
        );
        replays(&driver);
    }
    Ok(())
}
