//! Goal: Promise of Loyalty ("Each player puts a vow counter on a creature they control and sacrifices the rest. Each of those creatures can't attack you or planeswalkers you control for as long as it has a vow counter on it.") at two to four seats: each player keeps the creature they choose with a vow counter and sacrifices the others, a player with one creature keeps it unasked and one with none is not asked, the kept creatures can't attack the caster or the caster's planeswalkers while the other defenders stay open, taking the counter off ends the restriction at once and for good, a counter moved or copied to another creature restricts nothing new, a creature that changes controller still can't attack the caster, and every log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::CardsChoice;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::Duration;
use mtg_engine::effects::Modification;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{free_creature, walker};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn vow() -> CounterKind {
    CounterKind::Other("vow".into())
}

/// The creatures of seat `seat` a table starts with: three, one, or none.
fn creatures(seat: usize, held: usize) -> Vec<mtg_engine::PresetPermanent> {
    ["A", "B", "C"]
        .iter()
        .take(held)
        .map(|letter| ready(free_creature(&format!("{letter}{seat}"), 2, 2)))
        .collect()
}

/// Seat 0 holds Promise of Loyalty with five Plains and the planeswalker "Chandra"; seat `n` holds `held[n]` creatures
// named A, B, C and the seat number, and every other seat a creature "Raider" and the seat number in hand.
fn game(seats: usize, held: &[usize]) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand_named(0, ["Promise of Loyalty"]);
    for seat in 0..seats {
        if seat > 0 {
            scenario = scenario.hand(seat, [free_creature(&format!("Raider{seat}"), 1, 1)]);
        }
        let mut ours = creatures(seat, held[seat]);
        if seat == 0 {
            ours.extend((0..5).map(|_| ready(mtg_testkit::cards::plains())));
            ours.push(ready(walker("Chandra", 3)));
        }
        scenario = scenario.battlefield(seat, ours);
    }
    scenario
}

/// Casts Promise of Loyalty and resolves it: every player asked for a creature keeps the `keep{seat}` one.
fn promise(driver: &mut Driver, keep: &str) -> Result<usize, ScriptError> {
    driver.cast("Promise of Loyalty")?;
    let mut asked = 0;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::ChooseCards(options) => {
                asked += 1;
                let seat = decision.player.0 as usize;
                let name = format!("{keep}{seat}");
                let pick = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| {
                        driver
                            .state()
                            .object(*id)
                            .is_some_and(|o| o.card().name == name)
                    })
                    .ok_or_else(|| ScriptError::Stuck(format!("{name} is not offered")))?;
                driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("Promise of Loyalty".into()))
}

/// The names of the creatures `seat` controls, sorted.
fn creatures_of(driver: &Driver, seat: usize) -> Vec<String> {
    let mut names: Vec<String> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.controller() == pid(seat) && o.card().name != "Chandra")
        .filter(|o| o.card().types.contains(&mtg_core::card::CardType::Creature))
        .map(|o| o.card().name.clone())
        .collect();
    names.sort();
    names
}

fn vows(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .map_or(0, |p| p.counters_of(&vow()))
}

/// The defenders the creature `name` is offered on its controller's declaration; `None` when it is not offered.
fn defenders(driver: &Driver, name: &str) -> Option<Vec<AttackTarget>> {
    let DecisionKind::DeclareAttackers(options) = &driver.decision().ok()?.kind else {
        return None;
    };
    options
        .attackers
        .iter()
        .find(|option| {
            driver
                .state()
                .object(option.object)
                .is_some_and(|o| o.card().name == name)
        })
        .map(|option| option.defenders.clone())
}

/// What seat `controller`'s vowed creature is offered besides the caster's side: every other seat's player.
fn open(seats: usize, controller: usize) -> Vec<AttackTarget> {
    (1..seats)
        .filter(|seat| *seat != controller)
        .map(|seat| AttackTarget::Player(pid(seat)))
        .collect()
}

/// Puts the Raider of each other seat onto the battlefield: a creature the spell did not touch, so every seat has an
/// attacker to declare and the restriction is read against it.
fn enter_raiders(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for seat in 1..seats {
        let raider = id_in(driver, Zone::Hand(pid(seat)), &format!("Raider{seat}"));
        driver.manual_by(
            pid(seat),
            &ManualAction::Move {
                object: raider,
                to: ManualZone::Battlefield,
            },
        )?;
    }
    Ok(())
}

/// Plays on to the declaration of attackers of seat `seat` in the round after the spell.
fn to_declaration(driver: &mut Driver, seats: usize, seat: usize) -> Result<(), ScriptError> {
    let turn = 2 + u32::try_from(if seat == 0 { seats } else { seat }).unwrap();
    driver.idle_until(turn, Step::DeclareAttackers)
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn id_in(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == name)
        })
        .unwrap()
}

#[rstest]
fn each_player_keeps_the_creature_they_choose_with_a_vow_counter_and_sacrifices_the_rest(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let held = vec![3; seats];
    let setup = game(seats, &held).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, &held))?;
    assert_eq!(promise(&mut driver, "B")?, seats, "every player chose");
    for seat in 0..seats {
        assert_eq!(creatures_of(&driver, seat), [format!("B{seat}")]);
        assert_eq!(vows(&driver, &format!("B{seat}")), 1);
    }
    enter_raiders(&mut driver, seats)?;
    for seat in 1..seats {
        to_declaration(&mut driver, seats, seat)?;
        let mut expected = open(seats, seat);
        // Seat 0 and the planeswalker it controls are closed to the vowed creature.
        expected.sort_by_key(|target| format!("{target:?}"));
        let mut offered = defenders(&driver, &format!("B{seat}")).unwrap_or_default();
        offered.sort_by_key(|target| format!("{target:?}"));
        assert_eq!(offered, expected, "seat {seat}");
        // The creature the spell did not touch may attack the caster and the caster's planeswalker.
        let chandra = driver.id_of("Chandra")?;
        let free = defenders(&driver, &format!("Raider{seat}")).unwrap_or_default();
        assert!(free.contains(&AttackTarget::Player(pid(0))), "seat {seat}");
        assert!(
            free.contains(&AttackTarget::Planeswalker(chandra)),
            "seat {seat}"
        );
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::no_creature(&[3, 0, 3, 3], 0)]
#[case::one_creature(&[3, 1, 3, 3], 1)]
fn a_player_with_one_or_no_creature_is_not_asked(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: &[usize],
    #[case] kept: usize,
) -> Result<(), ScriptError> {
    let held = &held[..seats];
    let setup = game(seats, held).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, held))?;
    // The player with one creature or none is not asked, so one question fewer than the players.
    assert_eq!(promise(&mut driver, "B")?, seats - 1);
    let names = creatures_of(&driver, 1);
    if kept == 1 {
        assert_eq!(names, ["A1"], "the only creature stays");
        assert_eq!(vows(&driver, "A1"), 1);
    } else {
        assert_eq!(names, Vec::<String>::new());
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn taking_the_counter_off_ends_the_restriction_at_once_and_for_good(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] put_back: bool,
) -> Result<(), ScriptError> {
    let held = vec![2; seats];
    let setup = game(seats, &held).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, &held))?;
    promise(&mut driver, "B")?;
    let kept = driver.id_of("B1")?;
    let counters = |delta| ManualAction::Counters {
        object: kept,
        counter: vow(),
        delta,
    };
    let before = driver.state().effects().len();
    driver.manual_by(pid(1), &counters(-1))?;
    if put_back {
        // A counter put back later is a new counter: the effect that followed the first one is gone.
        driver.manual_by(pid(1), &counters(1))?;
    }
    assert!(
        driver.state().effects().len() < before,
        "the effect expired"
    );
    enter_raiders(&mut driver, seats)?;
    to_declaration(&mut driver, seats, 1)?;
    let offered = defenders(&driver, "B1").unwrap_or_default();
    assert!(
        offered.contains(&AttackTarget::Player(pid(0))),
        "{offered:?}"
    );
    // The other kept creatures stay restricted.
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_counter_moved_or_copied_to_another_creature_restricts_nothing_new(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] moved: bool,
) -> Result<(), ScriptError> {
    let held = vec![2; seats];
    let setup = game(seats, &held).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, &held))?;
    promise(&mut driver, "B")?;
    enter_raiders(&mut driver, seats)?;
    let spare = driver.id_of("Raider1")?;
    let counter = |object, delta| ManualAction::Counters {
        object,
        counter: vow(),
        delta,
    };
    if moved {
        let kept = driver.id_of("B1")?;
        driver.manual_by(pid(1), &counter(kept, -1))?;
    }
    driver.manual_by(pid(1), &counter(spare, 1))?;
    to_declaration(&mut driver, seats, 1)?;
    let closed = AttackTarget::Player(pid(0));
    let kept_offered = defenders(&driver, "B1").unwrap_or_default();
    assert_eq!(
        kept_offered.contains(&closed),
        moved,
        "the creature the effect names is free only when its counter left it"
    );
    let raider_offered = defenders(&driver, "Raider1").unwrap_or_default();
    assert!(
        raider_offered.contains(&closed),
        "a counter put on another creature restricts nothing"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_vowed_creature_that_changes_controller_still_cannot_attack_the_caster(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let held = vec![2; seats];
    let setup = game(seats, &held).setup().unwrap();
    let mut driver = Driver::from_scenario(game(seats, &held))?;
    promise(&mut driver, "B")?;
    enter_raiders(&mut driver, seats)?;
    let stolen = driver.id_of("B1")?;
    // Seat 2 takes it, or the caster does at a table of two.
    let thief = if seats > 2 { 2 } else { 0 };
    driver.add_effect(
        driver.on(stolen),
        Modification::SetController(pid(thief)),
        Duration::Forever,
    )?;
    to_declaration(&mut driver, seats, thief)?;
    let offered = defenders(&driver, "B1").unwrap_or_default();
    let mut expected: Vec<AttackTarget> = (0..seats)
        .filter(|seat| *seat != 0 && *seat != thief)
        .map(|seat| AttackTarget::Player(pid(seat)))
        .collect();
    let mut sorted = offered.clone();
    sorted.sort_by_key(|target| format!("{target:?}"));
    expected.sort_by_key(|target| format!("{target:?}"));
    assert_eq!(sorted, expected);
    replays(&driver, &setup);
    Ok(())
}
