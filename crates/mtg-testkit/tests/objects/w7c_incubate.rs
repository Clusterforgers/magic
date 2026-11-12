//! Goal: incubate plays whole at tables of two, three and four seats: an Incubator token with N +1/+1 counters, its transform for {2} into a 0/0 Phyrexian artifact creature that keeps the counters, a copy of the Incubator that has only its front face and does not transform, a 0/0 Phyrexian with no counters that dies, a token that leaves and is gone, an opponent who incubates, and the log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::replay;
use mtg_testkit::cards::{cost, land};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A free sorcery whose spell is the effects of `json`.
fn w7c_sorcery(name: &str, json: &str) -> Arc<CardDefinition> {
    let effects = serde_json::from_str(json).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects,
            }),
    )
}

/// Seat 0 holds `spells` and four Forests.
fn w7c_table(seats: usize, spells: Vec<Arc<CardDefinition>>) -> Scenario {
    Scenario::new(seats).hand(0, spells).battlefield(
        0,
        (0..4)
            .map(|_| ready(land(Color::Green)))
            .collect::<Vec<_>>(),
    )
}

/// Passes priority and pays automatically until nothing is on the stack.
fn w7c_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
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

/// An incubated token as (name, +1/+1 counters, power, toughness, whether it is a creature).
type W7cToken = (String, u32, Option<i32>, Option<i32>, bool);

/// The tokens on the battlefield that seat `seat` controls, as (name, +1/+1 counters, power, toughness, creature).
fn w7c_tokens(driver: &Driver, seat: usize) -> Vec<W7cToken> {
    let state = driver.state();
    let mut found: Vec<_> = state
        .objects()
        .filter(|object| object.controller() == pid(seat))
        .filter_map(|object| {
            let permanent = object.permanent()?;
            let chars = characteristics(state, object.id())?;
            let incubated = chars
                .has_subtype(&object.card().subtypes, &Subtype("Incubator".into()))
                || chars.has_subtype(&object.card().subtypes, &Subtype("Phyrexian".into()));
            incubated.then(|| {
                (
                    object.id(),
                    (
                        object.card().name.clone(),
                        permanent.counters_of(&CounterKind::PlusOnePlusOne),
                        chars.power,
                        chars.toughness,
                        chars.types.contains(CardType::Creature),
                    ),
                )
            })
        })
        .collect();
    found.sort_by_key(|(id, _)| *id);
    found.into_iter().map(|(_, token)| token).collect()
}

fn w7c_replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

fn incubator(counters: u32) -> W7cToken {
    ("Incubator".to_owned(), counters, None, None, false)
}

fn phyrexian(counters: u32) -> W7cToken {
    let size = i32::try_from(counters).unwrap();
    (
        "Phyrexian Token".to_owned(),
        counters,
        Some(size),
        Some(size),
        true,
    )
}

#[rstest]
fn an_incubator_is_made_with_its_counters_and_transforms_for_two(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 3)] count: u32,
) -> Result<(), ScriptError> {
    let json = format!(r#"[{{"Incubate":{{"count":{count}}}}}]"#);
    let scenario = w7c_table(seats, vec![w7c_sorcery("Hatch", &json)]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Hatch")?;
    w7c_resolve(&mut driver)?;
    assert_eq!(w7c_tokens(&driver, 0), [incubator(count)]);
    for seat in 1..seats {
        assert_eq!(w7c_tokens(&driver, seat).len(), 0);
    }
    driver.begin_activation("Incubator")?;
    w7c_resolve(&mut driver)?;
    if count == 0 {
        // A 0/0 Phyrexian with no counters dies to the state-based actions.
        assert_eq!(w7c_tokens(&driver, 0).len(), 0);
    } else {
        assert_eq!(w7c_tokens(&driver, 0), [phyrexian(count)]);
        let token = driver
            .state()
            .objects()
            .find(|o| o.card().name == "Phyrexian Token")
            .unwrap();
        let chars = characteristics(driver.state(), token.id()).unwrap();
        assert!(chars.types.contains(CardType::Artifact));
    }
    w7c_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn each_player_named_incubates(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let json = r#"[{"Incubate":{"who":"EachPlayer","count":2}}]"#;
    let scenario = w7c_table(seats, vec![w7c_sorcery("Brood", json)]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Brood")?;
    w7c_resolve(&mut driver)?;
    for seat in 0..seats {
        assert_eq!(w7c_tokens(&driver, seat), [incubator(2)], "seat {seat}");
    }
    w7c_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn a_copy_of_an_incubator_has_one_face_and_does_not_transform(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let copy =
        r#"[{"CreateToken":{"token":{"CopyOf":{"of":{"Each":[{"Subtype":"Incubator"}]}}}}}]"#;
    let scenario = w7c_table(
        seats,
        vec![
            w7c_sorcery("Hatch", r#"[{"Incubate":{"count":1}}]"#),
            w7c_sorcery("Mirror", copy),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Hatch")?;
    w7c_resolve(&mut driver)?;
    driver.cast("Mirror")?;
    w7c_resolve(&mut driver)?;
    // The copy has the copiable values of the Incubator, not its counters.
    assert_eq!(w7c_tokens(&driver, 0), [incubator(1), incubator(0)]);
    let ids: Vec<_> = driver
        .state()
        .objects()
        .filter(|o| o.card().name == "Incubator")
        .map(mtg_engine::object::GameObject::id)
        .collect();
    let copy_id = *ids.iter().max().unwrap();
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("seat 0 has priority")
    };
    let ability = options
        .abilities
        .iter()
        .find(|a| a.object == copy_id)
        .copied()
        .unwrap();
    driver.answer(Answer::Priority(PriorityAction::Activate(ability)))?;
    w7c_resolve(&mut driver)?;
    // A copy of a transforming token copies only its front face (CR 707.8a), so it does not transform.
    assert_eq!(w7c_tokens(&driver, 0), [incubator(1), incubator(0)]);
    w7c_replays(&driver, scenario);
    Ok(())
}

#[rstest]
fn an_incubator_that_leaves_is_gone(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let exile = r#"[{"Exile":{"what":{"Each":[{"Subtype":"Incubator"}]}}}]"#;
    let scenario = w7c_table(
        seats,
        vec![
            w7c_sorcery("Hatch", r#"[{"Incubate":{"count":2}}]"#),
            w7c_sorcery("Banish", exile),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.cast("Hatch")?;
    w7c_resolve(&mut driver)?;
    driver.cast("Banish")?;
    w7c_resolve(&mut driver)?;
    assert_eq!(w7c_tokens(&driver, 0).len(), 0);
    assert!(
        driver
            .state()
            .objects()
            .all(|o| o.card().name != "Incubator")
    );
    w7c_replays(&driver, scenario);
    Ok(())
}
