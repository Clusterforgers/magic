//! Goal: the corpus cards of phasing and devour play whole at tables of two, three and four seats: Breezekeeper phases out with the Auras on it (its own and an opponent's) before its controller untaps and in again at the next untap step of that player only, untapping then; Perch Protection with the gift phases out its own Bird tokens, which stay objects of the game through the opponent's extra turn and attack after phasing in; a phased-out commander stays the commander outside the command zone; a creature that phases out in combat leaves it (an attacker deals and takes no damage, an attacker whose blocker phased out stays blocked); the fused Gallifrey Falls // No More leaves the damaged creatures it phases out alive, their damage removed in the cleanup; Skullmulcher and Marrow Chomper read the creatures they devoured; and every log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands};
use crate::common::pid;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, CostsChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::format::Commander;
use mtg_engine::object::GameObject;
use mtg_engine::rules::life_total_cant_change;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{forest, free_creature, mountain, plains, swamp};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready, tapped};
use rstest::rstest;

/// Every seat gets a library of Forests, so that a game of several turns draws.
fn w10b_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    })
}

/// The objects named `name`, wherever they are, in id order.
fn w10b_all(driver: &Driver, name: &str) -> Vec<ObjectId> {
    let mut found: Vec<ObjectId> = driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(GameObject::id)
        .collect();
    found.sort();
    found
}

/// The one object named `name`, wherever it is (a phased-out permanent is not on the battlefield).
fn w10b_id(driver: &Driver, name: &str) -> ObjectId {
    let found = w10b_all(driver, name);
    assert_eq!(found.len(), 1, "one {name}");
    found[0]
}

fn w10b_zone(driver: &Driver, name: &str) -> Zone {
    driver
        .state()
        .object(w10b_id(driver, name))
        .unwrap()
        .zone()
}

fn w10b_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes until the stack is empty.
fn w10b_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The turn on which seat 0 next begins a turn, `rounds` rounds after the first.
fn w10b_turn_of_seat_0(seats: usize, rounds: usize) -> u32 {
    u32::try_from(seats * rounds + 1).unwrap()
}

#[rstest]
fn w10b_breezekeeper_phases_out_with_both_auras_and_in_again_for_its_controller_only(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] was_tapped: bool,
) -> Result<(), ScriptError> {
    let breezekeeper = card("Breezekeeper").unwrap();
    let mine = vec![
        if was_tapped {
            tapped(breezekeeper)
        } else {
            ready(breezekeeper)
        },
        attached_to(ready(card("Rancor").unwrap()), 0, 0),
    ];
    let scenario = Scenario::new(seats)
        .battlefield(0, mine)
        .battlefield(1, [attached_to(ready(card("Pacifism").unwrap()), 0, 0)]);
    let mut driver = Driver::from_scenario(w10b_libraries(scenario, seats))?;
    let counter = |driver: &Driver| {
        driver
            .state()
            .object(w10b_id(driver, "Breezekeeper"))
            .unwrap()
            .counter()
    };
    let before = counter(&driver);
    driver.idle_until(w10b_turn_of_seat_0(seats, 1), Step::Upkeep)?;
    for name in ["Breezekeeper", "Rancor", "Pacifism"] {
        assert_eq!(w10b_zone(&driver, name), Zone::PhasedOut, "{name}");
    }
    let pacifism = driver.state().object(w10b_id(&driver, "Pacifism")).unwrap();
    let phasing = pacifism.permanent().unwrap().phasing().unwrap();
    assert!(phasing.indirect && phasing.controller == pid(1));
    let held = driver.state().object(w10b_id(&driver, "Breezekeeper")).unwrap();
    assert_eq!(
        held.permanent().unwrap().tapped(),
        was_tapped,
        "what phases out does not untap"
    );
    // The untap step of the Pacifism's controller leaves it with the creature.
    driver.idle_until(w10b_turn_of_seat_0(seats, 1) + 1, Step::Upkeep)?;
    assert_eq!(w10b_zone(&driver, "Pacifism"), Zone::PhasedOut);
    driver.idle_until(w10b_turn_of_seat_0(seats, 2), Step::Upkeep)?;
    let keeper = w10b_id(&driver, "Breezekeeper");
    for name in ["Breezekeeper", "Rancor", "Pacifism"] {
        assert_eq!(w10b_zone(&driver, name), Zone::Battlefield, "{name}");
    }
    for aura in ["Rancor", "Pacifism"] {
        assert_eq!(
            driver.state().host_of(w10b_id(&driver, aura)),
            Some(keeper),
            "{aura}"
        );
    }
    assert_eq!(counter(&driver), before, "the same object");
    let held = driver.state().object(keeper).unwrap().permanent().unwrap();
    assert!(!held.tapped(), "it untaps with the rest as it phases in");
    assert!(driver.engine().events().iter().all(|event| !matches!(
        event,
        Event::ZoneChanged { object, .. } if *object == keeper
    )));
    w10b_replays(&driver);
    Ok(())
}

/// Answers the gift of Perch Protection: to seat 1, or to nobody.
fn w10b_promise(driver: &mut Driver, gift: bool) -> Result<(), ScriptError> {
    let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
        panic!("the gift is asked: {}", driver.decision()?.kind.name());
    };
    let costs = if gift {
        let offered = options
            .optional
            .iter()
            .find(|cost| cost.label.ends_with("to player 2"))
            .unwrap_or_else(|| panic!("a promise to seat 1: {:?}", options.optional));
        vec![offered.index]
    } else {
        vec![]
    };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)))?;
    Ok(())
}

#[rstest]
fn w10b_perch_protection_phases_out_its_tokens_through_the_extra_turn_of_the_gift(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] gift: bool,
) -> Result<(), ScriptError> {
    let mut mine = lands(6, plains);
    mine.push(ready(free_creature("Bear", 2, 2)));
    mine.push(attached_to(ready(card("Bonesplitter").unwrap()), 0, 6));
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Perch Protection"])
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Wolf", 3, 3))]);
    let mut driver = Driver::from_scenario(w10b_libraries(scenario, seats))?;
    driver.begin_cast("Perch Protection")?;
    w10b_promise(&mut driver, gift)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    w10b_resolve(&mut driver)?;
    let birds = w10b_all(&driver, "Bird");
    assert_eq!(birds.len(), 4);
    let out = if gift {
        Zone::PhasedOut
    } else {
        Zone::Battlefield
    };
    for bird in &birds {
        assert_eq!(driver.state().object(*bird).unwrap().zone(), out);
        assert!(!driver.state().has_ceased(*bird));
    }
    for name in ["Bear", "Bonesplitter"] {
        assert_eq!(w10b_zone(&driver, name), out, "{name}");
    }
    assert_eq!(w10b_zone(&driver, "Wolf"), Zone::Battlefield);
    assert_eq!(w10b_zone(&driver, "Perch Protection"), Zone::Exile);
    assert_eq!(life_total_cant_change(driver.state(), pid(0)), gift);
    // The extra turn of seat 1 comes first, then the regular turns; seat 0's things stay out through all of them.
    let back = w10b_turn_of_seat_0(seats, 1) + u32::from(gift);
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(driver.state().turn().active(), pid(1));
    driver.idle_until(back - 1, Step::Upkeep)?;
    assert_ne!(driver.state().turn().active(), pid(0));
    for bird in &birds {
        assert_eq!(driver.state().object(*bird).unwrap().zone(), out);
    }
    driver.idle_until(back, Step::DeclareAttackers)?;
    assert_eq!(driver.state().turn().active(), pid(0));
    assert_eq!(w10b_all(&driver, "Bird"), birds, "the same tokens");
    for bird in &birds {
        assert_eq!(
            driver.state().object(*bird).unwrap().zone(),
            Zone::Battlefield
        );
    }
    assert!(!life_total_cant_change(driver.state(), pid(0)));
    // The tokens were under seat 0's control all along: they attack.
    driver.attack(&[("Bird", 1), ("Bear", 1)])?;
    w10b_replays(&driver);
    Ok(())
}

/// Casts No More (the right half alone) at the creatures `names`, paying with Plains, and lets it resolve.
fn w10b_no_more(driver: &mut Driver, names: &[&str]) -> Result<(), ScriptError> {
    driver.begin_cast("Gallifrey Falls // No More")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        choose_way(driver, "No More")?;
    }
    let targets: Vec<_> = names
        .iter()
        .map(|name| driver.target_named(name))
        .collect::<Result<_, _>>()?;
    driver.choose_targets(&targets)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    w10b_resolve(driver)
}

#[rstest]
fn w10b_a_phased_out_commander_stays_the_commander_outside_the_command_zone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = lands(3, plains);
    mine.extend(lands(3, mountain));
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 2, &[mtg_core::color::Color::Red], 3, 3)])
        .hand_named(0, ["Gallifrey Falls // No More"])
        .battlefield(0, mine);
    let mut driver = Driver::from_scenario(w10b_libraries(scenario, seats))?;
    driver.cast_tapping("Cmd", &["Mountain", "Mountain", "Mountain"])?;
    w10b_resolve(&mut driver)?;
    let commander = driver.id_of("Cmd")?;
    w10b_no_more(&mut driver, &["Cmd"])?;
    assert_eq!(w10b_zone(&driver, "Cmd"), Zone::PhasedOut);
    assert!(driver.state().is_commander(commander));
    assert_eq!(driver.state().commander_casts(commander), 1);
    // Nothing offers to move it to the command zone, and seat 0 cannot cast it while it is out.
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(w10b_zone(&driver, "Cmd"), Zone::PhasedOut);
    driver.idle_until(w10b_turn_of_seat_0(seats, 1), Step::DeclareAttackers)?;
    assert_eq!(w10b_id(&driver, "Cmd"), commander);
    assert_eq!(w10b_zone(&driver, "Cmd"), Zone::Battlefield);
    driver.attack(&[("Cmd", 1)])?;
    driver.idle_until(w10b_turn_of_seat_0(seats, 1), Step::EndCombat)?;
    assert_eq!(driver.state().commander_damage(commander, pid(1)), 3);
    w10b_replays(&driver);
    Ok(())
}

/// Who phases out during the declare blockers step.
#[derive(Debug, Clone, Copy)]
enum W10bOut {
    /// The attacker, which seat 1 blocks.
    BlockedAttacker,
    /// The attacker, which nobody blocks.
    UnblockedAttacker,
    /// The blocker, by seat 1.
    Blocker,
}

#[rstest]
fn w10b_a_creature_that_phases_out_in_combat_leaves_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(W10bOut::BlockedAttacker, W10bOut::UnblockedAttacker, W10bOut::Blocker)] out: W10bOut,
) -> Result<(), ScriptError> {
    let mut mine = lands(3, plains);
    mine.push(ready(free_creature("Bear", 4, 4)));
    let mut theirs = lands(3, plains);
    theirs.push(ready(free_creature("Wolf", 3, 3)));
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Gallifrey Falls // No More"])
        .hand_named(1, ["Gallifrey Falls // No More"])
        .battlefield(0, mine)
        .battlefield(1, theirs);
    let mut driver = Driver::from_scenario(w10b_libraries(scenario, seats))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Bear", 1)])?;
    driver.pass_until(Step::DeclareBlockers)?;
    let blocked = !matches!(out, W10bOut::UnblockedAttacker);
    driver.block(if blocked { &[("Wolf", "Bear")] } else { &[] })?;
    match out {
        W10bOut::BlockedAttacker | W10bOut::UnblockedAttacker => {
            w10b_no_more(&mut driver, &["Bear"])?;
        }
        W10bOut::Blocker => {
            driver.pass()?;
            w10b_no_more(&mut driver, &["Wolf"])?;
        }
    }
    let attackers = driver.state().combat().attackers().len();
    assert_eq!(
        attackers,
        usize::from(matches!(out, W10bOut::Blocker)),
        "only an attacker that phased out leaves combat"
    );
    if let Some(attacker) = driver.state().combat().attackers().first() {
        assert!(attacker.blocked(), "it stays blocked");
        assert!(attacker.blockers().is_empty());
    }
    driver.pass_until(Step::EndCombat)?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 20);
    for name in ["Bear", "Wolf"] {
        let id = w10b_id(&driver, name);
        let object = driver.state().object(id).unwrap();
        assert_eq!(object.permanent().unwrap().damage(), 0, "{name}");
    }
    w10b_replays(&driver);
    Ok(())
}

#[rstest]
fn w10b_fused_gallifrey_falls_leaves_the_creatures_it_phases_out_alive(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = lands(6, mountain);
    mine.extend(lands(3, plains));
    mine.push(ready(free_creature("Bear", 2, 2)));
    mine.push(ready(free_creature("Giant", 5, 5)));
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Gallifrey Falls // No More"])
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Wolf", 3, 3))]);
    let mut driver = Driver::from_scenario(w10b_libraries(scenario, seats))?;
    driver.begin_cast("Gallifrey Falls // No More")?;
    choose_way(&mut driver, "Gallifrey Falls // No More")?;
    let bear = driver.target_named("Bear")?;
    driver.choose_targets(&[bear])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    w10b_resolve(&mut driver)?;
    let bear = w10b_id(&driver, "Bear");
    assert_eq!(w10b_zone(&driver, "Bear"), Zone::PhasedOut);
    let damage = |driver: &Driver, id: ObjectId| {
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .damage()
    };
    assert_eq!(damage(&driver, bear), 4, "dealt damage first, then phased out");
    assert_eq!(w10b_zone(&driver, "Wolf"), Zone::Exile);
    assert_eq!(w10b_zone(&driver, "Giant"), Zone::Battlefield);
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(damage(&driver, bear), 0, "the cleanup removes it from a phased-out permanent");
    driver.idle_until(w10b_turn_of_seat_0(seats, 1), Step::Draw)?;
    assert_eq!(w10b_zone(&driver, "Bear"), Zone::Battlefield);
    assert_eq!(w10b_id(&driver, "Bear"), bear);
    w10b_replays(&driver);
    Ok(())
}

/// Seat 0 holds `name`, controls the lands for it and `pets` 1/1 creatures, and devours `devoured` of them.
fn w10b_devour(
    seats: usize,
    name: &str,
    land: fn() -> Arc<mtg_cards::CardDefinition>,
    pets: usize,
    devoured: usize,
) -> Result<Driver, ScriptError> {
    let mut mine: Vec<PresetPermanent> = lands(4, land);
    mine.push(ready(forest()));
    mine.extend((0..pets).map(|_| ready(free_creature("Pet", 1, 1))));
    let scenario = Scenario::new(seats)
        .hand_named(0, [name])
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Foe", 1, 1))]);
    let mut driver = Driver::from_scenario(w10b_libraries(scenario, seats))?;
    driver.begin_cast(name)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..12 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                assert_eq!(options.from.len(), pets, "only seat 0's creatures");
                let chosen = options.from[..devoured].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                break;
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    Ok(driver)
}

#[rstest]
#[case::none(0, 0)]
#[case::one_of_two(2, 1)]
#[case::three(3, 3)]
fn w10b_skullmulcher_and_marrow_chomper_read_the_creatures_they_devoured(
    #[values(2, 3, 4)] seats: usize,
    #[case] pets: usize,
    #[case] devoured: usize,
) -> Result<(), ScriptError> {
    let hand = |driver: &Driver| driver.state().zone(Zone::Hand(pid(0))).len();
    let driver = w10b_devour(seats, "Skullmulcher", forest, pets, devoured)?;
    assert_eq!(hand(&driver), devoured, "a card for each creature devoured");
    assert_eq!(w10b_all(&driver, "Pet").len(), pets);
    w10b_replays(&driver);
    let driver = w10b_devour(seats, "Marrow Chomper", swamp, pets, devoured)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, 20 + 2 * i32::try_from(devoured).unwrap());
    let chomper = driver.id_of("Marrow Chomper")?;
    let counters = driver
        .state()
        .object(chomper)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne);
    assert_eq!(counters as usize, 2 * devoured);
    w10b_replays(&driver);
    Ok(())
}
