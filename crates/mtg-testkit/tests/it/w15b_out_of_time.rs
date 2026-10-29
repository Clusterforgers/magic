//! Goal: the time-counter cards play whole at tables of two, three and four seats: Out of Time untaps every creature, phases them all out with a time counter each, loses one each upkeep of its controller by vanishing without a number and brings them back as it is sacrificed; The Moment destroys each nonland permanent with mana value up to its time counters, itself included, and is sacrificed; The War Doctor deals its time counters as damage on attack and exiles the creature that would die, which puts a counter on it; a batch of cards exiled from the battlefield and a graveyard together triggers it once; and every log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::GameObject;
use mtg_engine::{PresetPermanent, replay};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

fn w15b_time() -> CounterKind {
    CounterKind::Other("time".into())
}

/// The first object named `name`, wherever it is.
fn w15b_named(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(GameObject::id)
        .min()
        .unwrap()
}

fn w15b_zone(driver: &Driver, name: &str) -> Zone {
    let id = w15b_named(driver, name);
    driver.state().object(id).unwrap().zone()
}

fn w15b_counters(driver: &Driver, name: &str) -> u32 {
    let id = w15b_named(driver, name);
    driver
        .state()
        .object(id)
        .and_then(GameObject::permanent)
        .map_or(0, |permanent| permanent.counters_of(&w15b_time()))
}

fn w15b_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Passes until the stack is empty, answering a target decision with `target`.
fn w15b_resolve(driver: &mut Driver, target: Option<&str>) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            let chosen = driver.target_named(target.expect("a target"))?;
            driver.choose_targets(&[chosen])?;
            continue;
        }
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// `permanent` beside three basic lands made by `land`.
fn w15b_with_lands(
    permanent: PresetPermanent,
    land: fn() -> Arc<CardDefinition>,
) -> Vec<PresetPermanent> {
    let mut all = vec![permanent];
    all.extend(lands(3, land));
    all
}

fn w15b_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..12).map(|_| forest()))
    })
}

#[rstest]
fn w15b_out_of_time_holds_every_creature_until_its_last_time_counter_goes(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Out of Time"])
        .battlefield(0, w15b_with_lands(tapped(free_creature("Ogre", 3, 3)), plains))
        .battlefield(1, [ready(free_creature("Wolf", 3, 3))]);
    let mut driver = Driver::from_scenario(w15b_libraries(scenario, seats))?;
    driver.cast_tapping("Out of Time", &["Plains", "Plains", "Plains"])?;
    w15b_resolve(&mut driver, None)?;
    assert_eq!(w15b_zone(&driver, "Ogre"), Zone::PhasedOut);
    assert_eq!(w15b_zone(&driver, "Wolf"), Zone::PhasedOut);
    assert_eq!(w15b_counters(&driver, "Out of Time"), 2, "one per creature");
    // Seat 0's next upkeep removes one; the one after removes the last and Out of Time is sacrificed.
    driver.idle_until(u32::try_from(seats + 1).unwrap(), Step::Main1)?;
    assert_eq!(w15b_counters(&driver, "Out of Time"), 1);
    assert_eq!(w15b_zone(&driver, "Wolf"), Zone::PhasedOut);
    driver.idle_until(u32::try_from(2 * seats + 1).unwrap(), Step::Main1)?;
    assert!(matches!(
        w15b_zone(&driver, "Out of Time"),
        Zone::Graveyard(_)
    ));
    for name in ["Ogre", "Wolf"] {
        assert_eq!(w15b_zone(&driver, name), Zone::Battlefield, "{name}");
    }
    let ogre = driver.state().object(w15b_named(&driver, "Ogre")).unwrap();
    assert!(!ogre.permanent().unwrap().tapped(), "untapped before it phased out");
    w15b_replays(&driver);
    Ok(())
}

#[rstest]
#[case::one(1, &["Llanowar Elves"])]
#[case::two(2, &["Llanowar Elves", "Grizzly Bears"])]
#[case::five(5, &["Llanowar Elves", "Grizzly Bears", "Serra Angel"])]
fn w15b_the_moment_destroys_up_to_its_time_counters_and_is_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[case] counters: u32,
    #[case] destroyed: &[&str],
) -> Result<(), ScriptError> {
    let moment = with_counters(ready(card("The Moment").unwrap()), w15b_time(), counters);
    let scenario = Scenario::new(seats)
        .battlefield(0, w15b_with_lands(moment, forest))
        .battlefield_named(
            seats - 1,
            ["Llanowar Elves", "Grizzly Bears", "Serra Angel"],
        );
    let mut driver = Driver::from_scenario(w15b_libraries(scenario, seats))?;
    driver.begin_ability("The Moment", 2)?;
    driver.pay_tapping(&["Forest", "Forest", "Forest"], 0)?;
    w15b_resolve(&mut driver, None)?;
    assert!(matches!(
        w15b_zone(&driver, "The Moment"),
        Zone::Graveyard(_)
    ));
    for name in ["Llanowar Elves", "Grizzly Bears", "Serra Angel"] {
        let gone = matches!(w15b_zone(&driver, name), Zone::Graveyard(_));
        assert_eq!(gone, destroyed.contains(&name), "{name}");
    }
    assert_eq!(
        driver.state().zone(Zone::Battlefield).len(),
        3 + 3 - destroyed.len(),
        "the lands and the costlier creatures stay"
    );
    w15b_replays(&driver);
    Ok(())
}

#[rstest]
fn w15b_the_war_doctor_exiles_the_creature_its_attack_trigger_kills(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let doctor = with_counters(ready(card("The War Doctor").unwrap()), w15b_time(), 3);
    let scenario = Scenario::new(seats)
        .battlefield(0, [doctor])
        .battlefield(seats - 1, [ready(free_creature("Wolf", 3, 3))]);
    let mut driver = Driver::from_scenario(w15b_libraries(scenario, seats))?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("The War Doctor", 1)])?;
    w15b_resolve(&mut driver, Some("Wolf"))?;
    assert_eq!(w15b_zone(&driver, "Wolf"), Zone::Exile, "exiled instead of dying");
    assert_eq!(
        w15b_counters(&driver, "The War Doctor"),
        4,
        "the exiled card puts a time counter on it"
    );
    w15b_replays(&driver);
    Ok(())
}

/// A free instant that exiles every creature an opponent owns on the battlefield and in the graveyards at once.
fn w15b_sweep() -> Arc<CardDefinition> {
    let zones = Pred::AnyOf(vec![
        Pred::In(ZoneKind::Battlefield),
        Pred::In(ZoneKind::Graveyard),
    ]);
    Arc::new(
        CardDefinition::new("Sweep")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![Effect::Exile {
                    what: Ref::Each(Selector(vec![
                        zones,
                        Pred::Type(CardType::Creature),
                        Pred::OwnedBy(Ref::Opponents),
                    ])),
                    bind: None,
                    duration: None,
                }],
            }),
    )
}

#[rstest]
fn w15b_cards_exiled_together_from_several_zones_trigger_the_war_doctor_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("The War Doctor").unwrap())])
        .hand(0, [w15b_sweep()])
        .battlefield(1, [ready(free_creature("Wolf", 3, 3))])
        .graveyard_named(seats - 1, ["Grizzly Bears", "Llanowar Elves"]);
    let mut driver = Driver::from_scenario(w15b_libraries(scenario, seats))?;
    driver.cast("Sweep")?;
    w15b_resolve(&mut driver, None)?;
    for name in ["Wolf", "Grizzly Bears", "Llanowar Elves"] {
        assert_eq!(w15b_zone(&driver, name), Zone::Exile, "{name}");
    }
    assert_eq!(w15b_counters(&driver, "The War Doctor"), 1, "one batch, one trigger");
    w15b_replays(&driver);
    Ok(())
}
