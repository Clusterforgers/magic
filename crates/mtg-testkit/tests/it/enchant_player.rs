//! Goal: an Aura that enchants a player plays as the rules say at tables of two, three and four seats: its spell is offered every player (or only the opponents of its controller for "enchant opponent") and enters attached to the one chosen in the batch that brings it, it goes to its owner's graveyard by state-based action when the player it enchants leaves the game or when an opponent-only Aura enchants its own controller, a Curse of Predation puts a counter on a creature that attacks the enchanted player and on no other, a Curse of the Forsaken gains its life for the attacker's controller, a Curse of the Nightly Hunt requires the creatures of the enchanted player to attack, an Aura's static ability and trigger name the enchanted player, and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics, replay};
use mtg_testkit::attach_cards::{blight_aura, spite_aura};
use mtg_testkit::cards::{bear, forest, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, enchanting, ready};
use rstest::rstest;

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn replays(driver: &Driver, setup: &mtg_engine::GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn lands() -> Vec<mtg_engine::setup::PresetPermanent> {
    vec![
        ready(forest()),
        ready(island()),
        ready(swamp()),
        ready(plains()),
        ready(forest()),
    ]
}

/// Seat 0 holds `hand` and has five lands and a Bear; every other seat has a Bear.
fn table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut mine = lands();
    mine.push(ready(bear()));
    let mut scenario = Scenario::new(seats).hand(0, hand).battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

fn counters(driver: &Driver, seat: usize) -> u32 {
    let bears: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            let object = driver.state().object(*id).unwrap();
            object.card().name == "Grizzly Bears" && object.controller() == pid(seat)
        })
        .collect();
    bears
        .iter()
        .map(|id| {
            driver
                .state()
                .object(*id)
                .unwrap()
                .permanent()
                .unwrap()
                .counters_of(&CounterKind::PlusOnePlusOne)
        })
        .sum()
}

fn on_the_battlefield(driver: &Driver, name: &str) -> bool {
    common::names(driver.state(), Zone::Battlefield)
        .iter()
        .any(|n| n == name)
}

// Casting

#[rstest]
#[case::enchant_player("Curse of Predation", false)]
#[case::enchant_opponent("Spite Aura", true)]
fn an_aura_is_offered_the_players_its_enchant_ability_allows(
    #[values(2, 3, 4)] seats: usize,
    #[case] aura: &str,
    #[case] opponents_only: bool,
) -> Result<(), ScriptError> {
    let hand = vec![card("Curse of Predation").unwrap(), spite_aura().unwrap()];
    let mut driver = Driver::from_scenario(table(seats, hand))?;
    driver.begin_cast(aura)?;
    let DecisionKind::ChooseTargets(options) = driver.decision()?.kind.clone() else {
        panic!("an Aura spell asks for its target as it is cast");
    };
    let mut offered: Vec<PlayerId> = options
        .legal
        .iter()
        .map(|target| target.player().expect("a player"))
        .collect();
    offered.sort();
    let expected: Vec<PlayerId> = (usize::from(opponents_only)..seats).map(pid).collect();
    assert_eq!(offered, expected);
    assert_eq!((options.min, options.max), (1, 1));
    Ok(())
}

#[rstest]
fn an_aura_enters_attached_to_the_player_it_targeted_in_the_batch_that_brings_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] enchanted: usize,
) -> Result<(), ScriptError> {
    let hand = || vec![card("Curse of Predation").unwrap()];
    let setup = table(seats, hand()).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, hand()))?;
    let target = driver.target_player(enchanted);
    driver.cast_at("Curse of Predation", &[target])?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    let at = events
        .iter()
        .position(|event| matches!(event, Event::SpellResolved { .. }))
        .unwrap();
    let (Event::SpellResolved { object }, Event::AttachedToPlayer { object: attached, player }) =
        (&events[at], &events[at + 1])
    else {
        panic!("the attachment follows the spell in the same batch: {events:?}");
    };
    assert_eq!(attached, object);
    assert_eq!(*player, pid(enchanted));
    let id = driver.id_of("Curse of Predation")?;
    assert_eq!(driver.state().player_host_of(id), Some(pid(enchanted)));
    assert_eq!(driver.state().host_of(id), None);
    assert_eq!(driver.state().attachments_of_player(pid(enchanted)), [id]);
    assert_eq!(driver.state().attachments_of_player(pid(seats - 1)).len(), usize::from(seats - 1 == enchanted));
    replays(&driver, &setup);
    Ok(())
}

// State-based actions

#[rstest]
fn an_aura_goes_to_the_graveyard_when_the_player_it_enchants_leaves_the_game(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats).battlefield(
            0,
            [
                enchanting(ready(card("Curse of Predation").unwrap()), 1),
                enchanting(ready(card("Curse of the Forsaken").unwrap()), 2),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    assert!(on_the_battlefield(&driver, "Curse of Predation"));
    driver.concede(pid(1))?;
    settle(&mut driver)?;
    assert!(!on_the_battlefield(&driver, "Curse of Predation"));
    assert!(on_the_battlefield(&driver, "Curse of the Forsaken"));
    assert_eq!(
        common::names(driver.state(), Zone::Graveyard(pid(0))),
        ["Curse of Predation"]
    );
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn an_aura_that_enchants_only_opponents_goes_to_the_graveyard_when_it_enchants_its_controller()
-> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2).battlefield(
            0,
            [
                enchanting(ready(spite_aura().unwrap()), 0),
                enchanting(ready(card("Curse of Predation").unwrap()), 0),
            ],
        ),
    )?;
    assert!(!on_the_battlefield(&driver, "Spite Aura"));
    assert!(on_the_battlefield(&driver, "Curse of Predation"));
    Ok(())
}

// The Curses

#[rstest]
fn a_creature_attacking_the_enchanted_player_gets_a_counter_and_no_other_does(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] attacked: usize,
) -> Result<(), ScriptError> {
    if attacked >= seats {
        return Ok(());
    }
    let scenario = || {
        Scenario::new(seats).battlefield(
            0,
            [ready(bear()), enchanting(ready(card("Curse of Predation").unwrap()), 1)],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", attacked)])?;
    settle(&mut driver)?;
    assert_eq!(counters(&driver, 0), u32::from(attacked == 1));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_controller_of_a_creature_attacking_the_enchanted_player_gains_a_life(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats).battlefield(
            0,
            [ready(bear()), enchanting(ready(card("Curse of the Forsaken").unwrap()), 1)],
        ),
    )?;
    let before = driver.state().player(pid(0)).unwrap().life();
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", 1)])?;
    settle(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), before + 1);
    Ok(())
}

#[rstest]
fn the_creatures_of_the_enchanted_player_must_attack_and_no_other(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // The Aura enchants the active player, so their Bear is required to attack; a Bear of the other seats is not asked.
    let driver_for = |enchanted: usize| -> Result<Vec<ObjectId>, ScriptError> {
        let mut driver = Driver::from_scenario(
            Scenario::new(seats)
                .battlefield(0, [ready(bear())])
                .battlefield(
                    1,
                    [enchanting(ready(card("Curse of the Nightly Hunt").unwrap()), enchanted)],
                ),
        )?;
        driver.pass_until(Step::DeclareAttackers)?;
        let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
            panic!("declare attackers");
        };
        Ok(options.required.clone())
    };
    assert_eq!(driver_for(0)?.len(), 1);
    assert!(driver_for(1)?.is_empty());
    Ok(())
}

// What the Aura says of the enchanted player

#[rstest]
fn an_aura_trigger_names_the_enchanted_player(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [enchanting(ready(spite_aura().unwrap()), 1)])
            .library(1, [bear(), bear()])
            .library(0, [bear(), bear()]),
    )?;
    let life = |driver: &Driver, seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let (mine, theirs) = (life(&driver, 0), life(&driver, 1));
    // Seat 0 plays first and draws nothing, and its own draws would cost it nothing; the enchanted seat's draw on the
    // next turn costs it a life.
    driver.pass_until(Step::Draw)?;
    settle(&mut driver)?;
    assert_eq!((life(&driver, 0), life(&driver, 1)), (mine, theirs - 1));
    Ok(())
}

#[rstest]
fn an_aura_static_ability_changes_the_creatures_of_the_enchanted_player_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [ready(bear()), enchanting(ready(blight_aura().unwrap()), 1)],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    let driver = Driver::from_scenario(scenario)?;
    let power_of = |seat: usize| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .filter(|id| driver.state().object(*id).unwrap().controller() == pid(seat))
            .filter_map(|id| characteristics(driver.state(), id))
            .find(|chars| chars.is_creature())
            .and_then(|chars| chars.power)
            .unwrap()
    };
    assert_eq!(power_of(0), 2);
    assert_eq!(power_of(1), 1, "the enchanted player's Bear is a 1/1");
    for seat in 2..seats {
        assert_eq!(power_of(seat), 2);
    }
    Ok(())
}
