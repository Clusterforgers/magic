//! Goal: a requirement to attack a planeswalker plays at tables of two, three and four seats: after Gideon Jura's "+2: During target opponent's next turn, creatures that player controls attack Gideon Jura if able." every creature that opponent controls in their next turn must attack Gideon and may attack nothing else, a creature that came under their control after the ability resolved included (the ruling: it doesn't lock in), another opponent's creatures stay free in their own turns, and the requirement is over after that turn; Gideon, Battle-Forged's "+2: Up to one target creature an opponent controls attacks Gideon during its controller's next turn if able." binds the one creature it targets and not its controller's other creatures; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{AttackOptions, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// The Gideon of the card, by its face: Gideon Jura, or the back face of Kytheon turned up.
fn gideon(name: &str) -> Arc<CardDefinition> {
    let card = card(name).unwrap();
    match card.faces.get(1) {
        Some(back) if name.starts_with("Kytheon") => Arc::new(back.clone()),
        _ => card,
    }
}

/// Seat 0 controls the planeswalker with 6 loyalty; every other seat controls a Bear and a Fox (seat `n` names them
/// "Bear n" and "Fox n").
fn table(seats: usize, walker: &str) -> Scenario {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [with_counters(
            ready(gideon(walker)),
            CounterKind::Loyalty,
            6,
        )],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Bear {seat}"), 2, 2)),
                ready(free_creature(&format!("Fox {seat}"), 1, 1)),
            ],
        );
    }
    scenario
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The attack options of the declare attackers step of `turn`.
fn attack_options(driver: &mut Driver, turn: u32) -> Result<AttackOptions, ScriptError> {
    driver.idle_until(turn, Step::DeclareAttackers)?;
    match &driver.decision()?.kind {
        DecisionKind::DeclareAttackers(options) => Ok(options.clone()),
        other => panic!("attackers were expected, not {}", other.name()),
    }
}

/// Whether the creature must attack, and what it may attack.
fn bound(driver: &Driver, options: &AttackOptions, name: &str) -> (bool, Vec<AttackTarget>) {
    let id = driver.id_of(name).unwrap();
    let defenders = options
        .attackers
        .iter()
        .find(|option| option.object == id)
        .map(|option| option.defenders.clone())
        .unwrap_or_default();
    (options.required.contains(&id), defenders)
}

fn assert_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let log = driver.engine().events().to_vec();
    assert_eq!(
        replay(&setup, &log).unwrap().state_hash(),
        driver.state().state_hash()
    );
    let resumed = Engine::resume(setup, log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

#[rstest]
fn gideon_juras_target_attacks_him_through_their_next_turn(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] target: usize,
) -> Result<(), ScriptError> {
    if target >= seats {
        return Ok(());
    }
    let mut driver = Driver::from_scenario(table(seats, "Gideon Jura"))?;
    let walker = driver.id_of("Gideon Jura")?;
    driver.begin_ability("Gideon Jura", 0)?;
    let opponent = driver.target_player(target);
    driver.choose_targets(&[opponent])?;
    settle(&mut driver)?;
    for seat in 1..seats {
        let turn = 1 + u32::try_from(seat).unwrap();
        let options = attack_options(&mut driver, turn)?;
        for name in [format!("Bear {seat}"), format!("Fox {seat}")] {
            let (required, defenders) = bound(&driver, &options, &name);
            if seat == target {
                assert!(required, "{name} must attack");
                assert_eq!(defenders, vec![AttackTarget::Planeswalker(walker)]);
            } else {
                assert!(!required, "{name} is free");
                assert!(defenders.len() > 1, "{name} may attack the players");
            }
        }
    }
    // The next turn of the target is free again.
    let again = 1 + u32::try_from(seats + target).unwrap();
    let options = attack_options(&mut driver, again)?;
    assert!(!bound(&driver, &options, &format!("Bear {target}")).0);
    assert_replays(&driver, table(seats, "Gideon Jura"));
    Ok(())
}

#[rstest]
fn gideon_battle_forged_binds_the_one_creature_it_targets(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] targets_one: bool,
) -> Result<(), ScriptError> {
    let name = "Kytheon, Hero of Akros // Gideon, Battle-Forged";
    let mut driver = Driver::from_scenario(table(seats, name))?;
    let walker = driver.id_of("Gideon, Battle-Forged")?;
    driver.begin_ability("Gideon, Battle-Forged", 0)?;
    let chosen = if targets_one {
        vec![driver.target_named(&format!("Bear {}", seats - 1))?]
    } else {
        Vec::new()
    };
    driver.choose_targets(&chosen)?;
    settle(&mut driver)?;
    let options = attack_options(&mut driver, u32::try_from(seats).unwrap())?;
    let (required, defenders) = bound(&driver, &options, &format!("Bear {}", seats - 1));
    assert_eq!(required, targets_one);
    if targets_one {
        assert_eq!(defenders, vec![AttackTarget::Planeswalker(walker)]);
    }
    assert!(!bound(&driver, &options, &format!("Fox {}", seats - 1)).0);
    assert_replays(&driver, table(seats, name));
    Ok(())
}
