//! Goal: corpus cards of pack r30-7b play as their rules say at two, three and four seats: Crag Saurian goes to the
//! controller of the source that dealt it damage; Heartwood Storyteller lets each opponent of the caster of a
//! noncreature spell draw, and not the caster; Combustion Man destroys the target unless its controller takes damage
//! equal to his power; Archfiend of Spite makes the controller of the source that dealt it damage lose that much life
//! unless they sacrifice that many permanents; Yuna, Grand Summoner puts as many +1/+1 counters as a dying permanent
//! had of every kind; Void Mirror counters a spell cast with no colored mana and lets one cast with colored mana be;
//! Ultima's blight counter takes the land's types; the Ring's temptations are counted up to its levels.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::cards::{creature, forest, free_creature, instant, pinger, wastes};
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
        "Cyclonus, the Saboteur // Cyclonus, Cybertronian Fighter",
        "Deathmist Raptor",
        "Elbrus, the Binding Blade // Withengar Unbound",
        "Heartwood Storyteller",
        "Perrie, the Pulverizer",
        "Raphael, Tag Team Tough",
        "Rufus Shinra",
        "Silver Surfer, Galactus's Herald",
        "Typhoid Mary, Fractured",
        "Frodo, Adventurous Hobbit",
        "Sam, Loyal Attendant",
        "Ultima, Origin of Oblivion",
        "Void Mirror",
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

#[rstest]
fn void_mirror_counters_a_spell_cast_without_colored_mana(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] colored: bool,
) -> Result<(), ScriptError> {
    let mirror = corpus().unwrap().card("Void Mirror").unwrap();
    let land = if colored { forest() } else { wastes() };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(1, [ready(mirror)])
            .battlefield(0, [ready(land)])
            .hand(0, [creature("Golem", 1, &[], 2, 2)]),
    )?;
    let land = if colored { "Forest" } else { "Wastes" };
    driver.cast_tapping("Golem", &[land])?;
    settle(&mut driver, "Golem", true)?;
    assert_eq!(on_battlefield(&driver, "Golem"), colored);
    Ok(())
}

#[rstest]
fn ultima_takes_the_types_of_the_blighted_land(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ultima = corpus()
        .unwrap()
        .card("Ultima, Origin of Oblivion")
        .unwrap();
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, [ready(ultima)])
            .battlefield(1, [ready(forest())]),
    )?;
    driver.attack(&[("Ultima, Origin of Oblivion", 1)])?;
    settle(&mut driver, "Forest", true)?;
    let land = driver.id_of("Forest")?;
    let blight = driver
        .state()
        .object(land)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Other("blight".to_owned()));
    assert_eq!(blight, 1);
    let traits = mtg_engine::characteristics(driver.state(), land).unwrap();
    assert!(traits.subtypes.is_empty(), "{:?}", traits.subtypes);
    Ok(())
}

/// A creature with "{0}: The Ring tempts you. Then if the Ring has tempted you two or more times this game, draw a card."
fn tempter() -> std::sync::Arc<mtg_cards::CardDefinition> {
    let ability: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{0}","effects":[{"RingTempts":{}},{"If":{"condition":{"Compare":{"left":{"RingTempted":"You"},"op":"Ge","right":2}},"then":[{"Draw":{}}]}}]}}"#,
    )
    .unwrap();
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Tempter")
            .with_type(mtg_core::card::CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(ability),
    )
}

#[rstest]
fn the_ring_counts_its_temptations(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield(0, [ready(tempter())]),
    )?;
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    let before = hand(&driver);
    driver.begin_activation("Tempter")?;
    settle(&mut driver, "Tempter", true)?;
    assert_eq!(hand(&driver), before, "one temptation draws nothing");
    driver.begin_activation("Tempter")?;
    settle(&mut driver, "Tempter", true)?;
    assert_eq!(hand(&driver), before + 1, "the second one draws");
    Ok(())
}
