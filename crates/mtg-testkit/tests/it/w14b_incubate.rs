//! Goal: the corpus cards of incubate play whole at tables of two, three and four seats: Sunfall exiles every creature and incubates their number (none for an empty board), Excise the Imperfect makes the controller of the permanent it exiles incubate its mana value, Brimaz incubates the mana value of a Phyrexian creature or an artifact creature spell its controller casts (not of another creature) and proliferates at an end step only after a Phyrexian died under its controller's control that turn; every log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::refs::{Pred, Selector, Targetable};
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{cost, creature, one_target, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// Seat 0 holds `hand`, and `lands` Plains and `mine` on the battlefield.
fn w14b_table(
    seats: usize,
    hand: Vec<Arc<CardDefinition>>,
    lands: usize,
    mine: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let permanents = (0..lands).map(|_| plains()).chain(mine).map(ready);
    Scenario::new(seats).hand(0, hand).battlefield(0, permanents.collect::<Vec<_>>())
}

/// Passes priority and pays automatically until nothing is on the stack.
fn w14b_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    panic!("the stack did not empty");
}

/// The +1/+1 counters on each Incubator token seat `seat` controls, in id order.
fn w14b_incubators(driver: &Driver, seat: usize) -> Vec<u32> {
    let mut found: Vec<_> = driver
        .state()
        .objects()
        .filter(|object| object.controller() == pid(seat) && object.card().name == "Incubator")
        .filter_map(|object| {
            let permanent = object.permanent()?;
            Some((object.id(), permanent.counters_of(&CounterKind::PlusOnePlusOne)))
        })
        .collect();
    found.sort_by_key(|(id, _)| *id);
    found.into_iter().map(|(_, counters)| counters).collect()
}

fn w14b_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// A creature of `generic` mana value, with a subtype or another card type when given.
fn w14b_creature(name: &str, generic: u32, subtype: Option<&str>, artifact: bool) -> Arc<CardDefinition> {
    let mut definition = (*creature(name, generic, &[], 2, 2)).clone();
    if let Some(subtype) = subtype {
        definition = definition.with_subtype(subtype);
    }
    if artifact {
        definition = definition.with_type(CardType::Artifact);
    }
    Arc::new(definition)
}

/// A free sorcery that destroys target creature.
fn w14b_destroy() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Unmake")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(vec![Pred::Type(
                    CardType::Creature,
                )])))],
                effects: serde_json::from_str(r#"[{"Destroy":{"what":{"Target":0}}}]"#).unwrap(),
            }),
    )
}

#[rstest]
fn w14b_sunfall_incubates_the_number_of_creatures_it_exiled(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] each: usize,
) -> Result<(), ScriptError> {
    let beasts = |seat: usize| -> Vec<Arc<CardDefinition>> {
        (0..each)
            .map(|n| creature(&format!("Beast {seat}.{n}"), 1, &[], 2, 2))
            .collect()
    };
    let mut scenario = w14b_table(seats, vec![card("Sunfall").unwrap()], 5, beasts(0));
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, beasts(seat).into_iter().map(ready).collect::<Vec<_>>());
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Sunfall")?;
    w14b_resolve(&mut driver)?;
    let exiled = u32::try_from(seats * each).unwrap();
    assert_eq!(w14b_incubators(&driver, 0), [exiled]);
    for seat in 1..seats {
        assert!(w14b_incubators(&driver, seat).is_empty());
    }
    let creatures_left = driver
        .state()
        .objects()
        .filter(|object| object.zone() == Zone::Battlefield && object.card().name.starts_with("Beast"))
        .count();
    assert_eq!(creatures_left, 0);
    w14b_replays(&driver);
    Ok(())
}

#[rstest]
fn w14b_excise_makes_the_controller_of_what_it_exiled_incubate(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2, 3)] owner: usize,
    #[values(0, 4)] mana_value: u32,
) -> Result<(), ScriptError> {
    let owner = owner % seats;
    let ogre = creature("Ogre", mana_value, &[], 3, 3);
    let excise = vec![card("Excise the Imperfect").unwrap()];
    let scenario = if owner == 0 {
        w14b_table(seats, excise, 3, vec![ogre])
    } else {
        w14b_table(seats, excise, 3, Vec::new()).battlefield(owner, vec![ready(ogre)])
    };
    let mut driver = Driver::from_scenario(scenario)?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Excise the Imperfect", &[ogre])?;
    w14b_resolve(&mut driver)?;
    let exiled = driver.state().zone(Zone::Exile).iter().any(|id| {
        driver.state().object(*id).is_some_and(|object| object.card().name == "Ogre")
    });
    assert!(exiled);
    for seat in 0..seats {
        let wanted: Vec<u32> = if seat == owner { vec![mana_value] } else { Vec::new() };
        assert_eq!(w14b_incubators(&driver, seat), wanted, "seat {seat}");
    }
    w14b_replays(&driver);
    Ok(())
}

#[rstest]
fn w14b_brimaz_incubates_for_a_phyrexian_creature_or_an_artifact_creature_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand = vec![
        w14b_creature("Phyrexian Scout", 2, Some("Phyrexian"), false),
        w14b_creature("Clockwork Golem", 3, None, true),
        w14b_creature("Grizzly", 1, None, false),
    ];
    let scenario = w14b_table(seats, hand, 6, vec![card("Brimaz, Blight of Oreskos").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    for name in ["Grizzly", "Phyrexian Scout", "Clockwork Golem"] {
        driver.cast(name)?;
        w14b_resolve(&mut driver)?;
    }
    assert_eq!(w14b_incubators(&driver, 0), [2, 3]);
    for seat in 1..seats {
        assert!(w14b_incubators(&driver, seat).is_empty());
    }
    w14b_replays(&driver);
    Ok(())
}

#[rstest]
fn w14b_brimaz_proliferates_only_after_a_phyrexian_died_under_its_controller(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] phyrexian_dies: bool,
) -> Result<(), ScriptError> {
    let mine = vec![
        card("Brimaz, Blight of Oreskos").unwrap(),
        w14b_creature("Phyrexian Scout", 2, Some("Phyrexian"), false),
        w14b_creature("Grizzly", 1, None, false),
    ];
    let scenario = w14b_table(seats, vec![w14b_destroy()], 0, mine);
    let mut driver = Driver::from_scenario(scenario)?;
    let victim = driver.target_named(if phyrexian_dies { "Phyrexian Scout" } else { "Grizzly" })?;
    driver.cast_at("Unmake", &[victim])?;
    w14b_resolve(&mut driver)?;
    // Through the end step of this turn and of the next player's turn, when nothing died.
    driver.idle_until(3, Step::Upkeep)?;
    let proliferated = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Proliferated { player } if *player == pid(0)))
        .count();
    assert_eq!(proliferated, usize::from(phyrexian_dies));
    w14b_replays(&driver);
    Ok(())
}
