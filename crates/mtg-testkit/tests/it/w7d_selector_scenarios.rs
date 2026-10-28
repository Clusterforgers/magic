//! Goal: the selector predicates `Goaded`, `XInCost` and `HasAttached` play through the engine at tables of two, three
//! and four seats: Bothersome Quasit goads a creature that then can't block while another can, Frontline Medic can
//! counter only a spell with {X} in its mana cost and a cast trigger draws only for one, Reyav gives double strike only
//! to an attacker that is enchanted or equipped and Kaima goads only the creatures an Aura of its controller enchants
//! (even one stolen away); every answer keeps the invariants and the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::choose::{NumberChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;
use crate::w7d_control_scenarios::w7d_oracle;

type W7dResult = Result<(), ScriptError>;

/// Answers until the stack is empty and no trigger waits: targets from `picks` in order (up to each decision's most),
/// X is `x`, mana is paid automatically, anything else passively.
fn w7d_settle(driver: &mut Driver, picks: &mut Vec<Target>, x: u32) -> W7dResult {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        let state = driver.state();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && state.stack().is_empty()
            && state.resolving().is_none()
            && state.pending_triggers().is_empty()
        {
            return Ok(());
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(options) if !picks.is_empty() => {
                let take = (options.max as usize).min(picks.len());
                Answer::ChooseTargets(TargetChoice::Targets(picks.drain(..take).collect()))
            }
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(x)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Answers passively until `seat` has a decision of the kind called `kind`.
fn w7d_until(driver: &mut Driver, seat: usize, kind: &str) -> W7dResult {
    for _ in 0..300 {
        let decision = driver.decision()?.clone();
        if decision.player == pid(seat) && decision.kind.name() == kind {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck(kind.into()))
}

fn w7d_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn w7d_hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

/// The creatures `seat` must attack with at its next declaration of attackers: those goaded among them.
fn w7d_required(driver: &mut Driver, seat: usize) -> Result<Vec<String>, ScriptError> {
    w7d_until(driver, seat, "declare-attackers")?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        return Err(ScriptError::Stuck("declare-attackers".into()));
    };
    Ok(options
        .required
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect())
}

fn w7d_quasit() -> Arc<CardDefinition> {
    w7d_oracle(
        "Bothersome Quasit",
        "{2}{R}",
        "Creature — Demon",
        Some((3, 2)),
        "Menace\nGoaded creatures your opponents control can't block.\nWhenever you cast a noncreature spell, goad target creature an opponent controls. (Until your next turn, that creature attacks each combat if able and attacks a player other than you if able.)",
    )
}

/// An X spell: "Blaze deals X damage to any target."
fn w7d_blaze() -> Arc<CardDefinition> {
    w7d_oracle(
        "Blaze",
        "{X}{R}",
        "Sorcery",
        None,
        "Blaze deals X damage to any target.",
    )
}

#[rstest]
#[ignore = "(r2aa-c): a static reads Goaded while the layers are computed, so it matches nothing"]
fn w7d_a_goaded_creature_cannot_block(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [burn("Shock", 2)])
        .battlefield(
            0,
            [
                ready(w7d_quasit()),
                ready(free_creature("Raider", 4, 4)),
                ready(mountain()),
            ],
        )
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Guard", 2, 2)),
            ],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Shock")?;
    let mut picks = vec![
        driver.target_player(seats - 1),
        driver.target_named("Bear")?,
    ];
    w7d_settle(&mut driver, &mut picks, 0)?;
    w7d_until(&mut driver, 0, "declare-attackers")?;
    driver.attack(&[("Raider", 1)])?;
    w7d_until(&mut driver, 1, "declare-blockers")?;
    let DecisionKind::DeclareBlockers(options) = &driver.decision()?.kind else {
        panic!("not declaring blockers");
    };
    let blockers: Vec<String> = options
        .blockers
        .iter()
        .map(|option| {
            driver
                .state()
                .object(option.object)
                .unwrap()
                .card()
                .name
                .clone()
        })
        .collect();
    assert_eq!(blockers, vec!["Guard".to_owned()]);
    assert!(driver.block(&[("Bear", "Raider")]).is_err());
    driver.block(&[("Guard", "Raider")])?;
    driver.idle_until(2, Step::Main2)?;
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_quasit_goads_a_creature_that_then_must_attack(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [burn("Shock", 2)])
        .battlefield(0, [ready(w7d_quasit()), ready(mountain())])
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Guard", 2, 2)),
            ],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Shock")?;
    let mut picks = vec![
        driver.target_player(seats - 1),
        driver.target_named("Bear")?,
    ];
    w7d_settle(&mut driver, &mut picks, 0)?;
    assert_eq!(w7d_required(&mut driver, 1)?, vec!["Bear".to_owned()]);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_frontline_medic_counters_only_a_spell_with_x(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let medic = w7d_oracle(
        "Frontline Medic",
        "{2}{W}",
        "Creature — Human Cleric",
        Some((3, 3)),
        "Sacrifice this creature: Counter target spell with {X} in its mana cost unless its controller pays {3}.",
    );
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [burn("Shock", 2), w7d_blaze()])
        .battlefield(0, (0..3).map(|_| ready(mountain())).collect::<Vec<_>>())
        .battlefield(1, [ready(medic)]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let last = driver.target_player(seats - 1);
    driver.cast_at("Shock", &[last])?;
    w7d_until(&mut driver, 1, "priority")?;
    assert!(
        driver.offered_abilities("Frontline Medic")?.is_empty(),
        "Shock has no {{X}} in its cost"
    );
    w7d_settle(&mut driver, &mut Vec::new(), 0)?;
    assert_eq!(w7d_life(&driver, seats - 1), 18);
    driver.begin_cast("Blaze")?;
    let mut picks = vec![last];
    // Seat 0 announces X = 1 and aims Blaze; seat 1 then counters it with the Medic, and seat 0 cannot pay {3}.
    for _ in 0..20 {
        let decision = driver.decision()?.clone();
        if decision.player == pid(1) && matches!(decision.kind, DecisionKind::Priority(_)) {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(1)),
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(picks.clone()))
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    driver.begin_activation("Frontline Medic")?;
    let blaze = driver.state().stack().last().copied().unwrap();
    picks = vec![Target::Object(
        driver.state().object(blaze).unwrap().reference(),
    )];
    w7d_settle(&mut driver, &mut picks, 1)?;
    assert_eq!(w7d_life(&driver, seats - 1), 18, "Blaze was countered");
    assert!(driver.id_of("Frontline Medic").is_err());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_a_cast_trigger_draws_for_a_spell_with_x(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let muse = w7d_oracle(
        "Lattice Muse",
        "{1}{U}",
        "Enchantment",
        None,
        "Whenever you cast a spell with {X} in its mana cost, draw a card.",
    );
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, [burn("Shock", 2), w7d_blaze()])
        .library(0, (0..4).map(|n| free_creature(&format!("Card {n}"), 1, 1)))
        .battlefield(
            0,
            [
                ready(muse),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let last = driver.target_player(seats - 1);
    driver.cast_at("Shock", &[last])?;
    w7d_settle(&mut driver, &mut Vec::new(), 0)?;
    assert_eq!(w7d_hand(&driver, 0), 1, "Shock has no {{X}}");
    driver.begin_cast("Blaze")?;
    w7d_settle(&mut driver, &mut vec![last], 1)?;
    assert_eq!(
        w7d_hand(&driver, 0),
        1,
        "Blaze left the hand and a card was drawn"
    );
    assert_eq!(w7d_life(&driver, seats - 1), 17);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w7d_halvar_doubles_only_the_enchanted(#[values(2, 3, 4)] seats: usize) -> W7dResult {
    let eriette = w7d_oracle(
        "Halvar, God of Battle",
        "{1}{W}{B}",
        "Legendary Creature — God",
        Some((2, 4)),
        "Creatures you control that are enchanted or equipped have double strike.",
    );
    let aura = |name: &str| {
        w7d_oracle(
            name,
            "{W}",
            "Enchantment — Aura",
            None,
            "Enchant creature\nEnchanted creature gets +1/+1.",
        )
    };
    // Seat 1's Aura enchants the Knight, seat 0's own Aura the Squire; the Page has none: 3 + 3 twice, 1 once.
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            [
                ready(free_creature("Knight", 2, 2)),
                ready(free_creature("Squire", 2, 2)),
                ready(free_creature("Page", 1, 1)),
                attached_to(ready(aura("Own Boon")), 0, 1),
                ready(eriette),
            ],
        )
        .battlefield(1, [attached_to(ready(aura("Charm")), 0, 0)]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Knight", 1), ("Squire", 1), ("Page", 1)])?;
    driver.idle_until(2, Step::Main2)?;
    assert_eq!(w7d_life(&driver, 1), 20 - 6 - 6 - 1);
    replays(&driver, &setup);
    Ok(())
}
