//! Goal: devour, casualty and offspring play whole at tables of two, three and four seats: devour asks which of the controller's own permanents of its kind to sacrifice (never the opponents', never another kind, nobody when there are none) and the creature enters with its counters for each; casualty offers its sacrifice only when a creature of power N or more can pay it and copies the spell once when the cost is paid, without a second copy for the copy; offspring offers its cost only when the lands can pay it and makes one 1/1 token copy when it enters, none for the token itself; and every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::lands;
use crate::common::{name_of, pid};
use mtg_cards::value::CounterKind;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, CostsChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{forest, free_creature, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The log replays to the live state.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0 holds `hand` and controls `permanents`; the other seats have a library and an artifact or a creature of their own.
fn table(seats: usize, hand: &[&str], permanents: Vec<PresetPermanent>) -> Scenario {
    let scenario = (1..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, hand.iter().copied())
            .battlefield(0, permanents),
        |scenario, seat| {
            scenario.battlefield(
                seat,
                [
                    ready(mtg_testkit::corpus::card("Bonesplitter").unwrap()),
                    ready(free_creature("Foe", 2, 2)),
                ],
            )
        },
    );
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter(|id| name_of(state, **id) == name)
        .count()
}

fn named(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| name_of(state, *id) == name)
        .collect()
}

fn pay_all(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    Ok(())
}

/// Passes priority until a decision other than priority is pending or the stack is empty.
fn run_until_asked(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..12 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if !priority || driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

fn counters(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

#[rstest]
#[case::nothing_to_sacrifice(0, None)]
#[case::one_kept(1, Some(0))]
#[case::one_sacrificed(1, Some(1))]
#[case::two_of_three(3, Some(2))]
#[case::all_three(3, Some(3))]
fn devour_sacrifices_the_chosen_creatures_and_enters_with_counters_for_each(
    #[case] pets: usize,
    #[case] sacrificed: Option<usize>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = lands(3, mountain);
    permanents.extend((0..pets).map(|_| ready(free_creature("Pet", 1, 1))));
    let mut driver = Driver::from_scenario(table(seats, &["Thunder-Thrash Elder"], permanents))?;
    driver.begin_cast("Thunder-Thrash Elder")?;
    pay_all(&mut driver)?;
    run_until_asked(&mut driver)?;
    let mut kept = pets;
    if let Some(sacrificed) = sacrificed {
        let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
            panic!(
                "the creatures to sacrifice are asked: {}",
                driver.decision()?.kind.name()
            );
        };
        assert_eq!(options.min, 0);
        assert_eq!(options.max as usize, pets);
        let mut names: Vec<String> = options
            .from
            .iter()
            .map(|id| name_of(driver.state(), *id))
            .collect();
        names.dedup();
        assert_eq!(
            names,
            ["Pet"],
            "only my own creatures, not the Elder or a foe"
        );
        let chosen = options.from[..sacrificed].to_vec();
        driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
        kept -= sacrificed;
    } else {
        assert!(
            !matches!(driver.decision()?.kind, DecisionKind::ChooseCards(_)),
            "nobody is asked when there is nothing to sacrifice"
        );
    }
    run_until_asked(&mut driver)?;
    let elder = driver.id_of("Thunder-Thrash Elder")?;
    let devoured = sacrificed.unwrap_or(0);
    assert_eq!(
        counters(&driver, elder),
        3 * u32::try_from(devoured).unwrap()
    );
    assert_eq!(count(&driver, Zone::Battlefield, "Pet"), kept);
    assert_eq!(count(&driver, Zone::Graveyard(pid(0)), "Pet"), devoured);
    let shown = mtg_engine::characteristics(driver.state(), elder).unwrap();
    assert_eq!(shown.power, Some(1 + 3 * i32::try_from(devoured).unwrap()));
    // The creatures were sacrificed as the Elder entered, before it was on the battlefield.
    let events = driver.engine().events();
    let entered = events
        .iter()
        .position(|event| matches!(event, Event::SpellResolved { object } if *object == elder));
    let last_sacrifice = events.iter().rposition(|event| {
        matches!(
            event,
            Event::ZoneChanged {
                cause: ZoneCause::Sacrificed,
                ..
            }
        )
    });
    if devoured > 0 {
        assert!(last_sacrifice.unwrap() < entered.unwrap());
    }
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn devour_of_a_kind_offers_only_my_permanents_of_that_kind(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = lands(4, plains);
    permanents.extend([
        ready(mtg_testkit::corpus::card("Bonesplitter").unwrap()),
        ready(free_creature("Pet", 1, 1)),
    ]);
    let mut driver = Driver::from_scenario(table(seats, &["Caprichrome"], permanents))?;
    driver.begin_cast("Caprichrome")?;
    pay_all(&mut driver)?;
    run_until_asked(&mut driver)?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!(
            "the artifacts to sacrifice are asked: {}",
            driver.decision()?.kind.name()
        );
    };
    let names: Vec<String> = options
        .from
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    assert_eq!(
        names,
        ["Bonesplitter"],
        "my artifact, not a foe's, a creature or the Caprichrome"
    );
    let chosen = options.from.clone();
    driver.answer(Answer::ChooseCards(CardsChoice(chosen)))?;
    run_until_asked(&mut driver)?;
    let creature = driver.id_of("Caprichrome")?;
    assert_eq!(counters(&driver, creature), 1);
    assert_eq!(count(&driver, Zone::Battlefield, "Pet"), 1);
    assert_eq!(count(&driver, Zone::Graveyard(pid(0)), "Bonesplitter"), 1);
    assert_replays(&driver);
    Ok(())
}

fn cast_join_the_maestros(
    seats: usize,
    creatures: &[(&str, i32)],
    pays: Option<bool>,
) -> Result<Driver, ScriptError> {
    let mut permanents = lands(5, swamp);
    permanents.extend(
        creatures
            .iter()
            .map(|(name, power)| ready(free_creature(name, *power, *power))),
    );
    let mut driver = Driver::from_scenario(table(seats, &["Join the Maestros"], permanents))?;
    driver.begin_cast("Join the Maestros")?;
    match pays {
        None => {
            assert!(
                !matches!(driver.decision()?.kind, DecisionKind::ChooseCosts(_)),
                "no creature can pay casualty 2"
            );
        }
        Some(paid) => {
            let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
                panic!("casualty is asked: {}", driver.decision()?.kind.name());
            };
            assert_eq!(options.optional.len(), 1);
            assert!(options.optional[0].label.starts_with("Casualty"));
            driver.answer(Answer::ChooseCosts(CostsChoice::Costs(if paid {
                vec![0]
            } else {
                vec![]
            })))?;
            if paid && creatures.iter().filter(|(_, power)| *power >= 2).count() > 1 {
                let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
                    panic!("the creature to sacrifice is asked");
                };
                let mut names: Vec<String> = options
                    .from
                    .iter()
                    .map(|id| name_of(driver.state(), *id))
                    .collect();
                names.sort();
                assert_eq!(names, ["Big", "Huge"], "only creatures of power 2 or more");
                let huge = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| name_of(driver.state(), *id) == "Huge")
                    .unwrap();
                driver.answer(Answer::ChooseCards(CardsChoice(vec![huge])))?;
            }
        }
    }
    pay_all(&mut driver)?;
    for _ in 0..8 {
        run_until_asked(&mut driver)?;
        if !driver.state().stack().is_empty() {
            driver.pass()?;
            driver.pass()?;
        }
    }
    Ok(driver)
}

#[rstest]
#[case::power_one_cannot_pay(&[("Pet", 1)], None, 1)]
#[case::power_two_declined(&[("Big", 2)], Some(false), 1)]
#[case::power_two_paid(&[("Big", 2)], Some(true), 2)]
#[case::power_three_paid(&[("Huge", 3)], Some(true), 2)]
#[case::one_of_two_can_pay(&[("Pet", 1), ("Big", 2)], Some(true), 2)]
fn casualty_is_offered_for_a_creature_of_enough_power_and_copies_the_spell_once_when_paid(
    #[case] creatures: &[(&str, i32)],
    #[case] pays: Option<bool>,
    #[case] ogres: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = cast_join_the_maestros(seats, creatures, pays)?;
    assert_eq!(
        count(&driver, Zone::Battlefield, "Ogre Warrior"),
        ogres,
        "one Ogre for the spell and one for its copy"
    );
    let events = driver.engine().events();
    let copies = events
        .iter()
        .filter(|event| matches!(event, Event::SpellCopied(_)))
        .count();
    let casts = events
        .iter()
        .filter(|event| matches!(event, Event::SpellCast { .. }))
        .count();
    assert_eq!(copies, ogres - 1, "the copy is made once, and not copied");
    assert_eq!(casts, 1, "the copy is not cast");
    let sacrificed = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    cause: ZoneCause::Sacrificed,
                    ..
                }
            )
        })
        .count();
    assert_eq!(sacrificed, ogres - 1);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn casualty_with_two_creatures_that_can_pay_sacrifices_the_one_picked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = cast_join_the_maestros(seats, &[("Big", 2), ("Huge", 3)], Some(true))?;
    assert_eq!(count(&driver, Zone::Battlefield, "Big"), 1);
    assert_eq!(count(&driver, Zone::Battlefield, "Huge"), 0);
    assert_eq!(count(&driver, Zone::Graveyard(pid(0)), "Huge"), 1);
    assert_eq!(count(&driver, Zone::Battlefield, "Ogre Warrior"), 2);
    assert_replays(&driver);
    Ok(())
}

fn cast_charger(seats: usize, mountains: usize, pays: Option<bool>) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        &["Steampath Charger"],
        lands(mountains, mountain),
    ))?;
    driver.begin_cast("Steampath Charger")?;
    match pays {
        None => assert!(
            !matches!(driver.decision()?.kind, DecisionKind::ChooseCosts(_)),
            "the lands cannot pay offspring"
        ),
        Some(paid) => {
            let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
                panic!("offspring is asked: {}", driver.decision()?.kind.name());
            };
            assert_eq!(options.optional[0].label, "Offspring {2}");
            driver.answer(Answer::ChooseCosts(CostsChoice::Costs(if paid {
                vec![0]
            } else {
                vec![]
            })))?;
        }
    }
    assert_eq!(
        driver.pay_options()?.cost.mana_value(),
        if pays == Some(true) { 4 } else { 2 }
    );
    pay_all(&mut driver)?;
    for _ in 0..4 {
        driver.pass()?;
        driver.pass()?;
    }
    Ok(driver)
}

#[rstest]
#[case::cannot_pay(2, None, 0)]
#[case::declined(4, Some(false), 0)]
#[case::paid(4, Some(true), 1)]
fn offspring_makes_one_one_one_token_copy_only_when_its_cost_was_paid(
    #[case] mountains: usize,
    #[case] pays: Option<bool>,
    #[case] tokens: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = cast_charger(seats, mountains, pays)?;
    assert_eq!(
        count(&driver, Zone::Battlefield, "Steampath Charger"),
        1 + tokens
    );
    let made: Vec<_> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::TokenCreated(token) => Some((token.controller.index(), token.name.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(made.len(), tokens, "the token makes no copy of its own");
    for (controller, name) in made {
        assert_eq!((controller, name.as_str()), (0, "Steampath Charger"));
    }
    let state = driver.state();
    for id in named(&driver, Zone::Battlefield, "Steampath Charger") {
        let shown = mtg_engine::characteristics(state, id).unwrap();
        let is_token = state
            .object(id)
            .is_some_and(|object| object.kind() == ObjectKind::Token);
        let size = if is_token { (1, 1) } else { (2, 1) };
        assert_eq!((shown.power, shown.toughness), (Some(size.0), Some(size.1)));
    }
    assert_replays(&driver);
    Ok(())
}
