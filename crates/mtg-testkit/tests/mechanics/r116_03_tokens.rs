//! Goal: at 2 to 4 seats, the tokens of the Star Trek Commander cards enter as their cards define them: a predefined
//! token named after a card (Black Lotus, Giant Badger, Lord of the Pit, Feroz's Ban) with its mana cost and its
//! abilities, and a tapped noncreature land token named New Planet that is a Planet with a mana ability (CR 111.4).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::counter::CounterKind;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Resolves what the activation put on the stack, passing and answering passively until the stack is empty.
fn r116_resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let busy =
            !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty();
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the activation".into()))
}

/// The permanent of this name on seat 0's battlefield.
fn r116_named(driver: &Driver, name: &str) -> Option<mtg_core::ids::ObjectId> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

#[rstest]
#[case::black_lotus("Arzakon", 1, "Black Lotus", 0, None)]
#[case::giant_badger("Greensleeves", 0, "Giant Badger", 3, Some(2))]
#[case::lord_of_the_pit("Thomil, the Destroyer", 2, "Lord of the Pit", 7, Some(7))]
#[case::ferozs_ban("Feroz, Ulgrotha's Warden", 2, "Feroz's Ban", 6, None)]
fn r116_a_loyalty_ability_creates_the_named_token(
    #[values(2, 3, 4)] seats: usize,
    #[case] walker: &str,
    #[case] ability: usize,
    #[case] token: &str,
    #[case] mana_value: u32,
    #[case] power: Option<i32>,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [with_counters(
            ready(card(walker).unwrap()),
            CounterKind::Loyalty,
            10,
        )],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability(walker, ability)?;
    r116_resolve(&mut driver)?;
    let id = r116_named(&driver, token).expect("the token entered");
    let state = driver.state();
    let object = state.object(id).unwrap();
    assert_eq!(object.controller(), pid(0));
    assert_eq!(object.mana_value(), mana_value);
    let made = characteristics(state, id).unwrap();
    assert_eq!(made.power, power);
    assert_eq!(made.types.contains(CardType::Creature), power.is_some());
    assert!(
        !object.card().rules.is_empty() || !object.card().abilities.is_empty(),
        "{token} has its abilities"
    );
    Ok(())
}

/// A free artifact whose ability is the token creation of the named card's spell, the step at `step`.
fn r116_maker(of: &str, step: usize) -> Arc<CardDefinition> {
    let rules = mtg_cards::builtin::library().unwrap();
    let source = rules
        .cards()
        .find(|card| card.name == of)
        .expect("the card is in the corpus");
    let Some(Ability::Spell { effects, .. }) = source.faces[0].abilities.first() else {
        panic!("{of} has a spell ability");
    };
    let made = serde_json::to_string(&effects[step]).unwrap();
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["SacrificeSelf"],"effects":[{made}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Planet Maker")
            .with_type(CardType::Artifact)
            .with_rules(ability),
    )
}

#[rstest]
fn r116_a_new_planet_enters_tapped_as_a_planet_land_with_a_mana_ability(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(r116_maker("Genesis Effect", 2))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_ability("Planet Maker", 0)?;
    r116_resolve(&mut driver)?;
    let id = r116_named(&driver, "New Planet").expect("the planet entered");
    let state = driver.state();
    let object = state.object(id).unwrap();
    assert!(object.tapped());
    assert_eq!(object.controller(), pid(0));
    let made = characteristics(state, id).unwrap();
    assert!(made.types.contains(CardType::Land));
    assert!(!made.types.contains(CardType::Creature));
    assert_eq!(made.power, None);
    assert!(made.subtypes.iter().any(|subtype| subtype.0 == "Planet"));
    assert_eq!(made.colors, mtg_core::color::ColorSet::EMPTY);
    assert_eq!(object.card().rules.len() + object.card().abilities.len(), 1);
    Ok(())
}
