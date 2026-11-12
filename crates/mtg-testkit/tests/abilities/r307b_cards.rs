//! Goal: corpus cards of pack r30-7b play as their rules say at two, three and four seats: Crag Saurian goes to the
//! controller of the source that dealt it damage; Heartwood Storyteller lets each opponent of the caster of a
//! noncreature spell draw, and not the caster; Combustion Man destroys the target unless its controller takes damage
//! equal to his power; Archfiend of Spite makes the controller of the source that dealt it damage lose that much life
//! unless they sacrifice that many permanents; Yuna, Grand Summoner puts as many +1/+1 counters as a dying permanent
//! had of every kind.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::cards::{free_creature, instant, pinger};
use mtg_testkit::corpus::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// Answers until the stack is empty and priority is back: targets go to the object named `target`, yes-or-no questions
/// get `yes`, mana is paid automatically, anything else passively. The events from here on.
fn settle(driver: &mut Driver, target: &str, yes: bool) -> Result<Vec<Event>, ScriptError> {
    let mut log = Vec::new();
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            return Ok(log);
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![driver.target_named(target)?]))
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(yes)),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            _ => Driver::passive_answer(&decision),
        };
        log.extend(driver.answer(answer)?);
    }
    Err(ScriptError::Stuck("the stack never emptied".to_owned()))
}

fn controller(driver: &Driver, name: &str) -> mtg_core::ids::PlayerId {
    let id = driver.id_of(name).unwrap();
    mtg_engine::characteristics::controller_of(driver.state(), id).unwrap()
}

#[rstest]
fn crag_saurian_goes_to_the_controller_of_the_source(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let saurian = corpus().unwrap().card("Crag Saurian").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 1, Step::Main1)
            .battlefield(0, [ready(saurian)])
            .battlefield(1, [ready(pinger("Pinger", 0))]),
    )?;
    assert_eq!(controller(&driver, "Crag Saurian"), pid(0));
    driver.begin_activation("Pinger")?;
    settle(&mut driver, "Crag Saurian", true)?;
    assert_eq!(controller(&driver, "Crag Saurian"), pid(1));
    Ok(())
}

#[rstest]
fn heartwood_storyteller_lets_the_casters_opponents_draw(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] yes: bool,
) -> Result<(), ScriptError> {
    let storyteller = corpus().unwrap().card("Heartwood Storyteller").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 1, Step::Main1)
            .battlefield(0, [ready(storyteller)])
            .hand(1, [instant("Quiet", 0, &[])]),
    )?;
    let hands: Vec<usize> = (0..seats)
        .map(|seat| driver.state().zone(Zone::Hand(pid(seat))).len())
        .collect();
    driver.cast("Quiet")?;
    settle(&mut driver, "Quiet", yes)?;
    for (seat, before) in hands.iter().enumerate() {
        let drew = usize::from(yes && seat != 1);
        let spent = usize::from(seat == 1);
        assert_eq!(
            driver.state().zone(Zone::Hand(pid(seat))).len(),
            before + drew - spent,
            "seat {seat}"
        );
    }
    Ok(())
}

#[rstest]
fn combustion_man_destroys_unless_its_controller_takes_his_power(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] take: bool,
) -> Result<(), ScriptError> {
    let combustion = corpus().unwrap().card("Combustion Man").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(combustion)])
            .battlefield(1, [ready(pinger("Pinger", 0))]),
    )?;
    let mut log = driver.attack(&[("Combustion Man", 1)])?;
    log.extend(settle(&mut driver, "Pinger", take)?);
    let alive = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| common::name_of(driver.state(), *id) == "Pinger");
    assert_eq!(alive, take, "{log:?}");
    let life = driver.state().player(pid(1)).unwrap().life();
    assert_eq!(life, if take { 16 } else { 20 });
    Ok(())
}

#[test]
fn every_card_of_the_pack_is_verified() {
    for name in [
        "Archfiend of Spite",
        "Archnemesis",
        "Combustion Man",
        "Crag Saurian",
        "Deathmist Raptor",
        "Elbrus, the Binding Blade // Withengar Unbound",
        "Heartwood Storyteller",
        "Perrie, the Pulverizer",
        "Raphael, Tag Team Tough",
        "Rufus Shinra",
        "Silver Surfer, Galactus's Herald",
        "Typhoid Mary, Fractured",
        "Ultra Magnus, Tactician // Ultra Magnus, Armored Carrier",
        "Yuna, Grand Summoner",
    ] {
        let support = corpus().unwrap().support(name).unwrap();
        assert_eq!(
            support.support,
            mtg_cards::support::Support::Verified,
            "{name}: {support:?}"
        );
    }
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| common::name_of(driver.state(), *id) == name)
}

#[rstest]
fn archfiend_of_spite_takes_life_unless_the_source_controller_sacrifices(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] sacrifice: bool,
) -> Result<(), ScriptError> {
    let archfiend = corpus().unwrap().card("Archfiend of Spite").unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 1, Step::Main1)
            .battlefield(0, [ready(archfiend)])
            .battlefield(1, [ready(pinger("Pinger", 0))]),
    )?;
    driver.begin_activation("Pinger")?;
    settle(&mut driver, "Archfiend of Spite", sacrifice)?;
    assert_eq!(on_battlefield(&driver, "Pinger"), !sacrifice);
    let life = driver.state().player(pid(1)).unwrap().life();
    assert_eq!(life, if sacrifice { 20 } else { 19 });
    Ok(())
}

#[rstest]
fn yuna_puts_as_many_counters_as_the_permanent_had(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let yuna = corpus().unwrap().card("Yuna, Grand Summoner").unwrap();
    let charged = with_counters(
        ready(free_creature("Charged", 1, 1)),
        CounterKind::Other("charge".to_owned()),
        3,
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(yuna), charged, ready(pinger("Pinger", 0))]),
    )?;
    driver.begin_activation("Pinger")?;
    // The pinger's target is the charged creature; Yuna's trigger then targets the pinger.
    let charged = driver.target_named("Charged")?;
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![charged])))?;
    settle(&mut driver, "Pinger", true)?;
    let pinger = driver.id_of("Pinger")?;
    let counters = driver
        .state()
        .object(pinger)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, 3);
    Ok(())
}
