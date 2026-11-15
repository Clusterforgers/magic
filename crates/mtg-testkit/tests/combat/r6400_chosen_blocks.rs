//! Goal: a rule that has another player choose which creatures block and how (CR 509.1a) plays at two to four seats:
//! under Invasion Plans the attacking player declares the defender's blocks, which still obey "All creatures block each
//! combat if able"; Odric with three other attackers gives its controller the declaration and with two does not; a rule
//! for a third player gives that player the declaration; the blocks are the defender's (`BlockersDeclared` names them),
//! a declaration the restrictions forbid is refused, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::DecisionKind;
use mtg_engine::effects::Duration;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Creatures with no abilities, each a 1/1.
fn creatures(names: &[&str]) -> Vec<PresetPermanent> {
    names
        .iter()
        .map(|name| ready(free_creature(name, 1, 1)))
        .collect()
}

/// Passes until blockers are declared; the player asked and whose blocks they declare.
fn to_blocks(driver: &mut Driver) -> Result<(usize, Option<usize>), ScriptError> {
    for _ in 0..40 {
        if let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind {
            // Whose creatures the declaration offers, when they are not the deciding player's.
            let asked = driver.decision()?.player;
            let whose = options
                .blockers
                .first()
                .and_then(|option| controller_of(driver.state(), option.object))
                .filter(|owner| *owner != asked)
                .map(mtg_core::ids::PlayerId::index);
            return Ok((asked.index(), whose));
        }
        driver.answer_passively()?;
    }
    panic!("blockers are declared");
}

/// The player the last `BlockersDeclared` names.
fn declared_for(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::BlockersDeclared { player, .. } => Some(player.index()),
            _ => None,
        })
        .unwrap()
}

/// The log replays to the same state.
fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
fn r6400_invasion_plans_gives_the_blocks_to_the_attacking_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] holder: usize,
) -> Result<(), ScriptError> {
    let mut theirs = creatures(&["Wall"]);
    let mut mine = creatures(&["Raider"]);
    if holder == 0 {
        mine.push(ready(card("Invasion Plans").unwrap()));
    } else {
        theirs.push(ready(card("Invasion Plans").unwrap()));
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, mine)
            .battlefield(1, theirs),
    )?;
    driver.attack(&[("Raider", 1)])?;
    assert_eq!(to_blocks(&mut driver)?, (0, Some(1)));
    assert!(driver.block(&[]).is_err(), "the Wall must block if able");
    driver.block(&[("Wall", "Raider")])?;
    assert_eq!(declared_for(&driver), 1);
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::three_others(3, true)]
#[case::two_others(2, false)]
fn r6400_odric_chooses_with_three_other_attackers(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: usize,
    #[case] chooses: bool,
) -> Result<(), ScriptError> {
    let names: Vec<String> = (0..others).map(|n| format!("Soldier {n}")).collect();
    let mut mine: Vec<PresetPermanent> = vec![ready(card("Odric, Master Tactician").unwrap())];
    mine.extend(names.iter().map(|name| ready(free_creature(name, 1, 1))));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, mine)
            .battlefield(1, creatures(&["Guard"])),
    )?;
    let mut attacks: Vec<(&str, usize)> = vec![("Odric, Master Tactician", 1)];
    attacks.extend(names.iter().map(|name| (name.as_str(), 1)));
    driver.attack(&attacks)?;
    let (asked, whose) = to_blocks(&mut driver)?;
    if chooses {
        assert_eq!((asked, whose), (0, Some(1)));
        driver.block(&[("Guard", "Soldier 0")])?;
        assert_eq!(declared_for(&driver), 1);
    } else {
        assert_eq!((asked, whose), (1, None));
        driver.block(&[])?;
    }
    replays(&driver);
    Ok(())
}

#[rstest]
fn r6400_a_third_player_chooses(#[values(3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, creatures(&["Raider"]))
            .battlefield(1, creatures(&["Guard"])),
    )?;
    driver.add_rule(
        Rule::R6400ChoosesBlocks {
            chooser: pid(2),
            whose: vec![pid(1)],
        },
        Duration::UntilEndOfTurn,
    )?;
    driver.attack(&[("Raider", 1)])?;
    assert_eq!(to_blocks(&mut driver)?, (2, Some(1)));
    driver.block(&[("Guard", "Raider")])?;
    assert_eq!(declared_for(&driver), 1);
    replays(&driver);
    Ok(())
}
