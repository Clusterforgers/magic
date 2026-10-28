//! Goal: exert plays through the engine at tables of two, three and four seats: a creature with exert is offered to be
//! exerted as it attacks and a refused exert leaves no trace, an exerted creature (Glorybringer) fires its "When you do"
//! and skips exactly one untap step of the player who exerted it while one not exerted untaps, a creature exerted under a
//! borrowed control untaps in its owner's untap step, vigilance leaves an exerted creature untapped, a creature exerted in
//! two combats skips one untap step and "Whenever you exert a creature" fires for each exert, "hasn't been exerted this
//! turn" offers the exert once, an exerted creature that dies leaves nothing behind, and bot games with exert keep the
//! invariants; Combat Celebrant untaps the others and adds one combat, Clockwork Droid can't be blocked and scries;
//! every log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{AttackOptions, DecisionKind};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::NothingToDo;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;
use crate::w7d_control_scenarios::w7d_oracle;

type W11aResult = Result<(), ScriptError>;

/// A 2/2 that may be exerted as it attacks and draws a card when it is, with `extra` before that line.
fn w11a_exerter(name: &str, extra: &str) -> Arc<CardDefinition> {
    w7d_oracle(
        name,
        "{1}{R}",
        "Creature — Human Warrior",
        Some((2, 2)),
        &format!("{extra}You may exert this creature as it attacks. When you do, draw a card."),
    )
}

/// A 2/2 with vigilance that may be exerted once each turn.
fn w11a_once(name: &str) -> Arc<CardDefinition> {
    w7d_oracle(
        name,
        "{1}{R}",
        "Creature — Human Warrior",
        Some((2, 2)),
        "Vigilance\nIf this creature hasn't been exerted this turn, you may exert it as it attacks.",
    )
}

/// An enchantment that draws a card whenever its controller exerts a creature.
fn w11a_watcher() -> Arc<CardDefinition> {
    w7d_oracle(
        "Exert Watcher",
        "{1}{R}",
        "Enchantment",
        None,
        "Whenever you exert a creature, you gain 1 life.",
    )
}

/// A sorcery that adds a combat after this main phase.
fn w11a_extra_combat() -> Arc<CardDefinition> {
    w7d_oracle(
        "Second Charge",
        "{R}",
        "Sorcery",
        None,
        "Untap all creatures you control. After this main phase, there is an additional combat phase followed by an additional main phase.",
    )
}

/// Answers passively until `seat` has a decision of the kind called `kind`.
fn w11a_until(driver: &mut Driver, seat: usize, kind: &str) -> W11aResult {
    for _ in 0..400 {
        let decision = driver.decision()?.clone();
        if decision.player == pid(seat) && decision.kind.name() == kind {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(ScriptError::Stuck(kind.into()))
}

/// The attack options of the pending declaration.
fn w11a_options(driver: &Driver) -> AttackOptions {
    let DecisionKind::DeclareAttackers(options) = driver.decision().unwrap().kind.clone() else {
        panic!("not declaring attackers");
    };
    options
}

/// Whether the attack options offer `object` to be exerted.
fn w11a_offered(options: &AttackOptions, object: ObjectId) -> bool {
    options
        .attackers
        .iter()
        .any(|option| option.object == object && option.exert)
}

/// Whether the permanent `name` is tapped.
fn w11a_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(|permanent| permanent.tapped())
}

/// How many events of the log `pick` counts.
fn w11a_count(driver: &Driver, pick: impl Fn(&Event) -> bool) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| pick(event))
        .count()
}

#[rstest]
fn w11a_glorybringer_skips_one_untap_step_of_its_exerter(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] exert: bool,
) -> W11aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("Glorybringer").unwrap())])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let glorybringer = driver.id_of("Glorybringer")?;
    let bear = driver.id_of("Bear")?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    assert!(w11a_offered(&w11a_options(&driver), glorybringer));
    if exert {
        driver.w11a_attack_exerting(&[("Glorybringer", 1)], &["Glorybringer"])?;
        w11a_until(&mut driver, 0, "choose-targets")?;
        let at = driver.target_named("Bear")?;
        driver.choose_targets(&[at])?;
    } else {
        driver.attack(&[("Glorybringer", 1)])?;
    }
    driver.idle_until(2, Step::Main2)?;
    assert_eq!(
        driver.state().object(bear).map(|object| object.zone()) == Some(Zone::Battlefield),
        !exert,
        "the Bear dies to the exert trigger only"
    );
    driver.idle_until(2 + seats as u32, Step::Upkeep)?;
    assert_eq!(w11a_tapped(&driver, "Glorybringer"), exert);
    let ended = |event: &Event| matches!(event, Event::ExertEnded { object, player } if *object == glorybringer && *player == pid(0));
    assert_eq!(w11a_count(&driver, ended), usize::from(exert));
    driver.idle_until(2 + 2 * seats as u32, Step::Upkeep)?;
    assert!(
        !w11a_tapped(&driver, "Glorybringer"),
        "skipped exactly once"
    );
    assert_eq!(w11a_count(&driver, ended), usize::from(exert));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w11a_an_exert_not_offered_is_refused_without_a_trace(
    #[values(2, 3, 4)] seats: usize,
) -> W11aResult {
    let scenario = Scenario::new(seats).at(2, 0, Step::Main1).battlefield(
        0,
        [
            ready(w11a_exerter("Exerter", "")),
            ready(free_creature("Wolf", 2, 2)),
        ],
    );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    let log = driver.engine().events().len();
    let refused = driver.w11a_attack_exerting(&[("Exerter", 1), ("Wolf", 1)], &["Wolf"]);
    assert!(matches!(
        refused,
        Err(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::NotOffered(_)
        )))
    ));
    let twice = driver.w11a_attack_exerting(&[("Exerter", 1)], &["Exerter", "Exerter"]);
    assert!(twice.is_err(), "an exert named twice is refused");
    assert_eq!(
        driver.engine().events().len(),
        log,
        "a refusal left a trace"
    );
    driver.w11a_attack_exerting(&[("Exerter", 1), ("Wolf", 1)], &["Exerter"])?;
    driver.idle_until(2, Step::Main2)?;
    let exerted =
        |event: &Event| matches!(event, Event::Exerted { player, .. } if *player == pid(0));
    assert_eq!(w11a_count(&driver, exerted), 1);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w11a_a_borrowed_exerted_creature_untaps_for_its_owner(
    #[values(2, 3, 4)] seats: usize,
) -> W11aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())])
        .battlefield(1, [ready(w11a_exerter("Exerter", ""))])
        .hand(0, [card("Act of Treason").unwrap()]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let exerter = driver.id_of("Exerter")?;
    let at = driver.target_named("Exerter")?;
    driver.cast_at("Act of Treason", &[at])?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    driver.w11a_attack_exerting(&[("Exerter", 1)], &["Exerter"])?;
    driver.idle_until(3, Step::Upkeep)?;
    assert!(
        !w11a_tapped(&driver, "Exerter"),
        "its owner's untap step untaps it"
    );
    let ended = |event: &Event| matches!(event, Event::ExertEnded { object, player } if *object == exerter && *player == pid(0));
    assert_eq!(w11a_count(&driver, ended), 0);
    driver.idle_until(2 + seats as u32, Step::Upkeep)?;
    assert_eq!(
        w11a_count(&driver, ended),
        1,
        "the exerter's untap step ends it"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w11a_two_combats_exert_twice_and_skip_one_untap_step(
    #[values(2, 3, 4)] seats: usize,
) -> W11aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(w11a_watcher()),
                ready(w11a_exerter("Vigil", "Vigilance\n")),
                ready(w11a_once("Celebrant")),
            ],
        )
        .hand(0, [w11a_extra_combat()]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let (vigil, celebrant) = (driver.id_of("Vigil")?, driver.id_of("Celebrant")?);
    driver.cast("Second Charge")?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    let options = w11a_options(&driver);
    assert!(w11a_offered(&options, vigil) && w11a_offered(&options, celebrant));
    driver.w11a_attack_exerting(&[("Vigil", 1), ("Celebrant", 1)], &["Vigil", "Celebrant"])?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    let options = w11a_options(&driver);
    assert!(
        w11a_offered(&options, vigil),
        "exert again in a second combat"
    );
    assert!(!w11a_offered(&options, celebrant), "once each turn");
    driver.w11a_attack_exerting(&[("Vigil", 1), ("Celebrant", 1)], &["Vigil"])?;
    driver.idle_until(2, Step::End)?;
    assert!(
        !w11a_tapped(&driver, "Vigil"),
        "vigilance leaves it untapped"
    );
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, 23, "the watcher fires for each of three exerts");
    driver.idle_until(2 + seats as u32, Step::Upkeep)?;
    let ended = |event: &Event| matches!(event, Event::ExertEnded { .. });
    assert_eq!(
        w11a_count(&driver, ended),
        2,
        "one end for each exerted creature"
    );
    let exerted = driver.state().object(vigil).unwrap().permanent().unwrap();
    assert!(exerted.exerted().is_empty());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w11a_an_exerted_creature_that_dies_leaves_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> W11aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(w11a_exerter("Exerter", ""))])
        .battlefield(1, [ready(free_creature("Ogre", 5, 5))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    driver.w11a_attack_exerting(&[("Exerter", 1)], &["Exerter"])?;
    w11a_until(&mut driver, 1, "declare-blockers")?;
    driver.block(&[("Ogre", "Exerter")])?;
    driver.idle_until(2 + seats as u32, Step::Upkeep)?;
    let ended = |event: &Event| matches!(event, Event::ExertEnded { .. });
    assert_eq!(w11a_count(&driver, ended), 0);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w11a_bots_exert_and_keep_the_invariants(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) -> W11aResult {
    let mut scenario = Scenario::new(seats).seed(seed).at(2, 0, Step::Main1);
    for seat in 0..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(card("Glorybringer").unwrap()),
                ready(w11a_exerter("Exerter", "")),
                ready(w11a_once("Celebrant")),
                ready(free_creature("Ox", 1, 4)),
            ],
        );
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let mut bots: Vec<RandomBot> = (0..seats as u64)
        .map(|seat| RandomBot::new(seed, seat))
        .collect();
    for _ in 0..1500 {
        if driver.state().turn().number() > 10 {
            break;
        }
        let Ok(decision) = driver.decision().cloned() else {
            break;
        };
        let answer = bots[decision.player.index()].choose(&decision);
        driver.answer(answer)?;
    }
    let exerted = |event: &Event| matches!(event, Event::Exerted { .. });
    assert!(
        w11a_count(&driver, exerted) > 0,
        "no creature was ever exerted"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w11a_combat_celebrant_untaps_the_others_and_adds_one_combat(
    #[values(2, 3, 4)] seats: usize,
) -> W11aResult {
    let scenario = Scenario::new(seats).at(2, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Combat Celebrant").unwrap()),
            ready(free_creature("Wolf", 2, 2)),
        ],
    );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let celebrant = driver.id_of("Combat Celebrant")?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    driver.w11a_attack_exerting(
        &[("Combat Celebrant", 1), ("Wolf", 1)],
        &["Combat Celebrant"],
    )?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    assert!(
        !w11a_tapped(&driver, "Wolf"),
        "the other creatures untapped"
    );
    let options = w11a_options(&driver);
    assert!(
        !w11a_offered(&options, celebrant),
        "tapped, and exerted this turn"
    );
    driver.attack(&[("Wolf", 1)])?;
    driver.idle_until(2, Step::End)?;
    assert_eq!(
        driver.state().player(pid(1)).unwrap().life(),
        12,
        "four and two, then two"
    );
    assert_eq!(
        driver.state().turn().combats(),
        2,
        "one combat more, not two"
    );
    driver.idle_until(2 + seats as u32, Step::Upkeep)?;
    assert!(w11a_tapped(&driver, "Combat Celebrant"));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w11a_clockwork_droid_cant_be_blocked_and_scries(#[values(2, 3, 4)] seats: usize) -> W11aResult {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(card("Clockwork Droid").unwrap())])
        .battlefield(1, [ready(free_creature("Wall", 0, 8))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    w11a_until(&mut driver, 0, "declare-attackers")?;
    driver.w11a_attack_exerting(&[("Clockwork Droid", 1)], &["Clockwork Droid"])?;
    driver.idle_until(2, Step::End)?;
    let blocked = |event: &Event| matches!(event, Event::NothingToDo { player, kind: NothingToDo::Block } if *player == pid(1));
    assert_eq!(
        w11a_count(&driver, blocked),
        1,
        "it can't be blocked this turn"
    );
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 17);
    let scried = |event: &Event| matches!(event, Event::Scried(_));
    assert_eq!(w11a_count(&driver, scried), 1);
    replays(&driver, &setup);
    Ok(())
}
