//! Goal: afflict plays whole at tables of two, three and four seats as its rules say: the defending player of a blocked creature loses its count of life (once for each instance, and only that player), an unblocked creature costs nobody life, and the life every opponent lost in the turn is what Neheb, the Eternal turns into red mana at the beginning of the postcombat main phase, added up over the opponents.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_cards::keyword::Keyword;
use mtg_engine::Event;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, keyworded_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, corpus, ready};
use rstest::rstest;

/// A creature that deals no damage and has these afflict counts.
fn afflicter(name: &str, counts: &[u32]) -> std::sync::Arc<mtg_cards::CardDefinition> {
    let keywords: Vec<Keyword> = counts.iter().copied().map(Keyword::Afflict).collect();
    keyworded_creature(name, 0, &[], 0, 3, &keywords)
}

/// Seat 0 controls these attackers, ready; every other seat has a Wall that deals no damage.
fn table(seats: usize, attackers: Vec<std::sync::Arc<mtg_cards::CardDefinition>>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, attackers.into_iter().map(ready).collect::<Vec<_>>());
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Wall {seat}"), 0, 4))]);
    }
    scenario
}

/// Seat 0 attacks with `attacks` (name, seat attacked); the walls named in `blocks` block (seat, attacker). The game goes
/// on until `until` with an empty stack.
fn fight(
    driver: &mut Driver,
    attacks: &[(&str, usize)],
    blocks: &[(usize, &str)],
    until: Step,
) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(attacks)?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                let wall = format!("Wall {}", decision.player.index());
                let chosen: Vec<(&str, &str)> = blocks
                    .iter()
                    .filter(|(seat, _)| *seat == decision.player.index())
                    .map(|(_, attacker)| (wall.as_str(), *attacker))
                    .collect();
                driver.block(&chosen)?;
            }
            DecisionKind::Priority(_)
                if driver.state().turn().step() == until && driver.state().stack().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the game did not reach {until:?}");
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn the_defending_player_of_a_blocked_creature_loses_life_once_for_each_instance(
    #[values(2, 3, 4)] seats: usize,
    #[values(&[1], &[2], &[1, 2])] counts: &[u32],
    #[values(true, false)] blocked: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, vec![afflicter("Afflicter", counts)]))?;
    let blocks: Vec<(usize, &str)> = if blocked {
        vec![(1, "Afflicter")]
    } else {
        Vec::new()
    };
    fight(&mut driver, &[("Afflicter", 1)], &blocks, Step::EndCombat)?;
    let lost = if blocked {
        i32::try_from(counts.iter().sum::<u32>()).unwrap()
    } else {
        0
    };
    assert_eq!(life(&driver, 1), 20 - lost);
    for seat in [0].into_iter().chain(2..seats) {
        assert_eq!(life(&driver, seat), 20, "seat {seat} lost life");
    }
    Ok(())
}

#[rstest]
fn each_blocked_creature_afflicts_the_player_it_attacks(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let attackers = vec![afflicter("Afflicter", &[2]), afflicter("Other", &[3])];
    let mut driver = Driver::from_scenario(table(seats, attackers))?;
    fight(
        &mut driver,
        &[("Afflicter", 1), ("Other", 2)],
        &[(1, "Afflicter"), (2, "Other")],
        Step::EndCombat,
    )?;
    assert_eq!(life(&driver, 1), 18);
    assert_eq!(life(&driver, 2), 17);
    Ok(())
}

/// The red mana Neheb gave seat 0 in the game so far.
fn red_added(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::ManaAdded { player, .. } if *player == pid(0)))
        .count()
}

#[rstest]
fn neheb_adds_a_red_mana_for_each_life_the_opponents_lost_this_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pinger: bool,
) -> Result<(), ScriptError> {
    let neheb = corpus::card("Neheb, the Eternal").unwrap();
    let mut attackers = vec![neheb];
    let mut attacks = vec![("Neheb, the Eternal", 1)];
    let mut blocks = vec![(1, "Neheb, the Eternal")];
    // Neheb is blocked by the Wall of seat 1 and costs it 3 life; a second afflicter costs the next seat 2 more.
    let second = pinger && seats > 2;
    if second {
        attackers.push(afflicter("Pinger", &[2]));
        attacks.push(("Pinger", 2));
        blocks.push((2, "Pinger"));
    }
    let mut driver = Driver::from_scenario(table(seats, attackers))?;
    assert_eq!(red_added(&driver), 0);
    fight(&mut driver, &attacks, &blocks, Step::Main2)?;
    assert_eq!(red_added(&driver), if second { 5 } else { 3 });
    Ok(())
}

#[test]
fn neheb_adds_nothing_when_no_opponent_lost_life() -> Result<(), ScriptError> {
    let neheb = corpus::card("Neheb, the Eternal").unwrap();
    let mut driver = Driver::from_scenario(table(2, vec![neheb]))?;
    fight(&mut driver, &[], &[], Step::Main2)?;
    assert_eq!(red_added(&driver), 0);
    Ok(())
}
