//! Goal: "blocked or was blocked [by a <kind>] this turn" selects both creatures of each block at two to four seats, the
//! kind judged as the blocking creature was when it blocked (one that died since still counts): Heat Stroke destroys
//! every creature in a block at end of combat and spares the unblocked attacker and the idle creature, Time to Reflect
//! and You Cannot Pass! offer only the creature on the other side of a block with a Zombie or a legendary creature, and
//! the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::Supertype;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature, a Zombie or legendary when `kind` says so.
fn r379_creature(name: &str, power: i32, toughness: i32, kind: &str) -> Arc<CardDefinition> {
    let base = (*free_creature(name, power, toughness)).clone();
    Arc::new(match kind {
        "Zombie" => base.with_subtype("Zombie"),
        "legendary" => base.with_supertype(Supertype::Legendary),
        _ => base,
    })
}

/// Seat 0 has the Raider (1/4) and the Idle (1/4), with `extra` beside them; seat 1 has the Guard (0/`guard`) and the
/// Rest (0/4). `raider` and `guard` are the kinds of those two.
fn r379_table(
    seats: usize,
    (raider, guard_kind, guard): (&str, &str, i32),
    extra: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let mut ours = vec![
        r379_creature("Raider", 1, 4, raider),
        r379_creature("Idle", 1, 4, ""),
    ];
    ours.extend(extra);
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, ours.into_iter().map(ready).collect::<Vec<_>>())
        .battlefield(
            1,
            [
                ready(r379_creature("Guard", 0, guard, guard_kind)),
                ready(r379_creature("Rest", 0, 4, "")),
            ],
        )
}

/// Seat 0 attacks seat 1 with the Raider and the Idle, the Guard blocks the Raider, and the game goes on to the
/// postcombat main phase with an empty stack.
fn r379_fight(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Raider", 1), ("Idle", 1)])?;
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) if decision.player.index() == 1 => {
                driver.block(&[("Guard", "Raider")])?;
            }
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::Main2
                    && driver.state().stack().is_empty() =>
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
    panic!("the game did not reach the postcombat main phase");
}

fn r379_alive(driver: &Driver, name: &str) -> bool {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

fn r379_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn r379_heat_stroke_destroys_each_creature_in_a_block(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || r379_table(seats, ("", "", 4), vec![card("Heat Stroke").unwrap()]);
    let mut driver = Driver::from_scenario(table())?;
    r379_fight(&mut driver)?;
    assert!(!r379_alive(&driver, "Raider"), "it was blocked");
    assert!(!r379_alive(&driver, "Guard"), "it blocked");
    assert!(r379_alive(&driver, "Idle"), "it attacked unblocked");
    assert!(r379_alive(&driver, "Rest"), "it did not block");
    r379_replays(&driver, table());
    Ok(())
}

#[rstest]
#[case::reflect_the_raider_is_a_zombie("Time to Reflect", ("Zombie", "", 4), Some("Guard"))]
#[case::reflect_the_guard_is_a_zombie("Time to Reflect", ("", "Zombie", 4), Some("Raider"))]
#[case::reflect_the_zombie_guard_died("Time to Reflect", ("", "Zombie", 1), Some("Raider"))]
#[case::reflect_near_miss_no_zombie("Time to Reflect", ("legendary", "", 4), None)]
#[case::cannot_pass_a_legendary_raider("You Cannot Pass!", ("legendary", "", 4), Some("Guard"))]
#[case::cannot_pass_a_legendary_guard("You Cannot Pass!", ("", "legendary", 4), Some("Raider"))]
#[case::cannot_pass_near_miss_a_zombie("You Cannot Pass!", ("Zombie", "", 4), None)]
fn r379_a_spell_targets_the_other_side_of_a_block(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] kinds: (&str, &str, i32),
    #[case] target: Option<&str>,
) -> Result<(), ScriptError> {
    let table =
        || r379_table(seats, kinds, vec![plains(), plains()]).hand(0, [card(spell).unwrap()]);
    let mut driver = Driver::from_scenario(table())?;
    r379_fight(&mut driver)?;
    let Some(target) = target else {
        assert!(
            driver.begin_cast(spell).is_err(),
            "no creature can be targeted"
        );
        return Ok(());
    };
    driver.begin_cast(spell)?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("the target is asked");
    };
    let wanted = driver.target_named(target)?;
    assert_eq!(options.legal, vec![wanted], "only the {target}");
    driver.choose_targets(&[wanted])?;
    for _ in 0..10 {
        if driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        } else {
            driver.answer_passively()?;
        }
    }
    assert!(!r379_alive(&driver, target), "{target} is gone");
    assert!(r379_alive(&driver, "Idle"), "the Idle stays");
    r379_replays(&driver, table());
    Ok(())
}
