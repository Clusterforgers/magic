//! Goal: the r30-7c combat cards play as printed at tables of two, three and four seats: Heroes in a Half Shell triggers
//! once for each player its Mutants, Ninjas and Turtles hurt, puts a +1/+1 counter on each creature of that batch (never on
//! another kind or a creature that hurt another player) and draws a card each time; Master of Cruelties makes an
//! unblocked attack set the player's life total to 1 with no damage after it, while a blocked one does nothing; Dimir
//! Strandcatcher surveils one card for each opponent being attacked; and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::bear;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A vanilla 2/2 creature card of the creature type `subtype`, named after it.
fn r307c_kind(subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(format!("Plain {subtype}"))
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(2, 2),
    )
}

/// Declares `attackers`, then plays to the end step: the defending seat blocks with `blocks` when asked, every other
/// question is answered passively.
fn r307c_fight(
    driver: &mut Driver,
    attackers: &[(&str, usize)],
    blocks: &[(&str, &str)],
) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(attackers)?;
    for _ in 0..120 {
        if driver.state().turn().step() == Step::End {
            return Ok(());
        }
        match driver.decision()?.kind.clone() {
            DecisionKind::DeclareBlockers(_) if !blocks.is_empty() => {
                driver.block(blocks)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat did not end".to_owned()))
}

fn r307c_counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

fn r307c_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn r307c_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Heroes in a Half Shell attacks with a Ninja, a Turtle and a Bear; `heroes_at` and `turtle_at` say which seat the Heroes
/// and the Turtle attack (the Ninja and the Bear attack seat 1). Expected: the counters on the Heroes, the Ninja and the
/// Turtle, and the cards drawn.
#[rstest]
#[case::one_player_one_batch(1, 1, [1, 1, 1], 1)]
#[case::two_players_two_batches(2, 1, [1, 1, 1], 2)]
#[case::the_turtle_alone_at_another(1, 2, [1, 1, 1], 2)]
fn r307c_heroes_count_each_creature_of_the_batch_once(
    #[values(3, 4)] seats: usize,
    #[case] heroes_at: usize,
    #[case] turtle_at: usize,
    #[case] counters: [u32; 3],
    #[case] draws: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .seed(5)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Heroes in a Half Shell").unwrap()),
                ready(r307c_kind("Ninja")),
                ready(r307c_kind("Turtle")),
                ready(bear()),
            ],
        )
        .library_named(0, ["Forest"; 6]);
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    r307c_fight(
        &mut driver,
        &[
            ("Heroes in a Half Shell", heroes_at),
            ("Plain Ninja", 1),
            ("Plain Turtle", turtle_at),
            ("Grizzly Bears", 1),
        ],
        &[],
    )?;
    let got = [
        r307c_counters(&driver, "Heroes in a Half Shell"),
        r307c_counters(&driver, "Plain Ninja"),
        r307c_counters(&driver, "Plain Turtle"),
    ];
    assert_eq!(got, counters);
    assert_eq!(r307c_counters(&driver, "Grizzly Bears"), 0);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand + draws);
    r307c_replays(&driver);
    Ok(())
}

/// Near miss: a batch of only Bears is no Mutant, Ninja or Turtle and does nothing.
#[rstest]
fn r307c_heroes_ignore_other_kinds(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Heroes in a Half Shell").unwrap()),
                ready(bear()),
            ],
        )
        .library_named(0, ["Forest"; 6]);
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    r307c_fight(&mut driver, &[("Grizzly Bears", 1)], &[])?;
    assert_eq!(r307c_counters(&driver, "Grizzly Bears"), 0);
    assert_eq!(r307c_counters(&driver, "Heroes in a Half Shell"), 0);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), hand);
    r307c_replays(&driver);
    Ok(())
}

/// Master of Cruelties attacks seat 1, which blocks with a Bear when `blocked`.
#[rstest]
fn r307c_master_sets_life_to_one_when_unblocked(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] blocked: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card("Master of Cruelties").unwrap())])
        .battlefield(1, [ready(bear())]);
    let mut driver = Driver::from_scenario(scenario)?;
    let before = r307c_life(&driver, 1);
    let blocks: &[(&str, &str)] = if blocked {
        &[("Grizzly Bears", "Master of Cruelties")]
    } else {
        &[]
    };
    r307c_fight(&mut driver, &[("Master of Cruelties", 1)], blocks)?;
    let expected = if blocked { before } else { 1 };
    assert_eq!(r307c_life(&driver, 1), expected);
    for seat in 2..seats {
        assert_eq!(r307c_life(&driver, seat), before);
    }
    r307c_replays(&driver);
    Ok(())
}

/// The cards Dimir Strandcatcher's controller surveiled, in all.
fn r307c_surveiled(driver: &Driver) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            mtg_engine::Event::Surveilled(surveilled) => {
                Some(surveilled.top.len() + surveilled.graveyard.len())
            }
            _ => None,
        })
        .sum()
}

/// Dimir Strandcatcher surveils once for each opponent being attacked, however many creatures attack each.
#[rstest]
#[case::one_opponent(2, 1, 1, 1)]
#[case::one_opponent_of_three(3, 1, 1, 1)]
#[case::two_opponents(3, 1, 2, 2)]
#[case::two_of_four(4, 2, 3, 2)]
fn r307c_strandcatcher_surveils_per_opponent_attacked(
    #[case] seats: usize,
    #[case] bear_at: usize,
    #[case] ninja_at: usize,
    #[case] surveiled: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Dimir Strandcatcher").unwrap()),
                ready(bear()),
                ready(r307c_kind("Ninja")),
            ],
        )
        .library_named(0, ["Forest"; 6]);
    let mut driver = Driver::from_scenario(scenario)?;
    r307c_fight(
        &mut driver,
        &[("Grizzly Bears", bear_at), ("Plain Ninja", ninja_at)],
        &[],
    )?;
    assert_eq!(r307c_surveiled(&driver), surveiled);
    r307c_replays(&driver);
    Ok(())
}
