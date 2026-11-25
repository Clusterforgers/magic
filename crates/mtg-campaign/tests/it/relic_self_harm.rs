//! Goal: no relic of the current generation empties its holder's library: a boss seat casting a cheap instant (mana
//! value 0 to 2, from its Satchel or its hand) under any relic keeps every card of its library at tables of two to four;
//! a run before generation 16 keeps the Maelstrom Crown that exiles a library holding nothing cheaper, so its boss falls
//! at its next draw as it did.

use std::sync::Arc;

use mtg_campaign::run::{CONTENT, CROWN_CASCADE_FROM};
use mtg_campaign::{CardKind, content};
use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, instant, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Cards in the holder's library: lands and creatures of mana value 3, nothing a cheap instant cascades into.
const LIBRARY: usize = 30;

/// A table whose last seat, the boss, is active in its first main phase with `relics`, six Mountains and `spell` in its
/// Satchel or hand over a library of [`LIBRARY`] cards; the heroes have a library of Mountains.
fn table(
    seats: usize,
    relics: &[Arc<CardDefinition>],
    spell: Arc<CardDefinition>,
    satchel: bool,
) -> Scenario {
    let boss = seats - 1;
    let library = (0..LIBRARY).map(|i| {
        if i % 3 == 1 {
            creature("Ogre", 3, &[], 3, 3)
        } else {
            mountain()
        }
    });
    let mut scenario = Scenario::new(seats)
        .at(1, boss, Step::Main1)
        .life(boss, 55)
        .battlefield(boss, (0..6).map(|_| ready(mountain())))
        .library(boss, library)
        .emblems(boss, relics.iter().cloned());
    scenario = if satchel {
        scenario.satchel_reusable(boss, [spell])
    } else {
        scenario.hand(boss, [spell])
    };
    for seat in 0..boss {
        scenario = scenario
            .life(seat, 40)
            .library(seat, (0..40).map(|_| mountain()));
    }
    scenario
}

fn resolve_saying_yes(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The boss's library after it casts `spell` under `relics`.
fn library_after(
    seats: usize,
    relics: &[Arc<CardDefinition>],
    spell: Arc<CardDefinition>,
    satchel: bool,
) -> Result<Driver, ScriptError> {
    let name = spell.name.clone();
    let mut driver = Driver::from_scenario(table(seats, relics, spell, satchel))?;
    driver.cast(&name)?;
    resolve_saying_yes(&mut driver)?;
    Ok(driver)
}

fn library(driver: &Driver, seats: usize) -> usize {
    driver
        .state()
        .zone(Zone::Library(PlayerId::from_index(seats - 1)))
        .len()
}

#[rstest]
fn no_relic_of_the_generation_empties_its_holders_library(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] mana_value: u32,
    #[values(true, false)] satchel: bool,
) -> Result<(), ScriptError> {
    let content = content().unwrap();
    for relic in content.relics() {
        if relic.team || !relic.in_generation(CONTENT) {
            continue;
        }
        let spell = instant("Quick", mana_value, &[]);
        let driver = library_after(seats, &[relic.definition()], spell, satchel)?;
        assert_eq!(
            library(&driver, seats),
            LIBRARY,
            "{}: a {mana_value}-mana instant at {seats} seats",
            relic.id
        );
    }
    Ok(())
}

#[rstest]
fn an_older_run_keeps_the_crown_that_exiles_a_library_with_nothing_cheaper(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] mana_value: u32,
    #[values(true, false)] satchel: bool,
) -> Result<(), ScriptError> {
    let content = content().unwrap();
    let old = content.card("maelstrom_crown").unwrap();
    assert!(old.in_generation(CROWN_CASCADE_FROM - 1));
    assert!(!old.in_generation(CROWN_CASCADE_FROM));
    let spell = instant("Quick", mana_value, &[]);
    let mut driver = library_after(seats, &[old.definition()], spell, satchel)?;
    assert_eq!(library(&driver, seats), 0, "the whole library is exiled");
    // The boss falls at its next draw, at full life: the round-2 loss of the deep Warrens.
    let boss = PlayerId::from_index(seats - 1);
    for _ in 0..2000 {
        if !driver.state().player(boss).unwrap().is_in_game() {
            break;
        }
        driver.answer_passively()?;
    }
    let player = driver.state().player(boss).unwrap();
    assert!(
        !player.is_in_game(),
        "the boss loses drawing from an empty library"
    );
    assert_eq!(player.life(), 55);
    Ok(())
}

#[test]
fn the_crown_of_the_generation_cascades_and_puts_the_cards_passed_on_the_bottom()
-> Result<(), ScriptError> {
    let content = content().unwrap();
    let crowns: Vec<_> = content
        .relics()
        .into_iter()
        .filter(|c| c.name == "Maelstrom Crown" && c.in_generation(CONTENT))
        .collect();
    assert_eq!(crowns.len(), 1, "one Crown a generation");
    assert_eq!(crowns[0].kind, CardKind::Relic);
    let driver = library_after(3, &[crowns[0].definition()], instant("Quick", 4, &[]), true)?;
    // The first Ogre (mana value 3) is cast; the cards passed go to the bottom.
    assert_eq!(library(&driver, 3), LIBRARY - 1);
    assert_eq!(
        driver.state().zone(Zone::Exile).len(),
        0,
        "nothing stays in exile"
    );
    let ogres = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.card().name == "Ogre")
        .count();
    assert_eq!(ogres, 1, "the card found is cast free");
    Ok(())
}
