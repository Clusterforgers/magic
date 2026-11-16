//! Goal: a static ability that names a permanent (Witness Protection, layer 3) replaces every name it had, at 2 to 4
//! seats: the permanent is read by its new name by a static ability over that name and by the legend rule (two
//! legends given one name leave one, CR 704.5j), keeps its supertypes, has the colors, types and size the Aura gives and
//! no abilities, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::corpus::card;
use mtg_testkit::trigger_cards::sized_creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, ready};
use rstest::rstest;

use crate::common::copying::replays;

const NEW_NAME: &str = "Legitimate Businessperson";

/// A 1/1 with "Creatures named Legitimate Businessperson get +1/+1."
fn admirer() -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"Named":"{NEW_NAME}"}}]}},"mods":[{{"ModifyPT":{{"power":1,"toughness":1}}}}]}}}}"#
    );
    sized_creature_with(
        "Admirer",
        1,
        1,
        vec![serde_json::from_str::<Ability>(&json).unwrap()],
    )
}

/// Seat 0 controls the legends Alpha and Beta, the Admirer, and a Witness Protection on each of the first `auras`
/// legends.
fn table(seats: usize, auras: usize) -> Scenario {
    let mut permanents = vec![
        ready(legend("Alpha", 1, &[Color::Red], 3, 3)),
        ready(legend("Beta", 1, &[Color::Blue], 4, 4)),
        ready(admirer()),
    ];
    for place in 0..auras {
        permanents.push(attached_to(
            ready(card("Witness Protection").unwrap()),
            0,
            place,
        ));
    }
    Scenario::new(seats).battlefield(0, permanents)
}

/// The legends seat 0 still controls, by the name the game reads.
fn legends(driver: &Driver) -> Vec<String> {
    let state = driver.state();
    let mut found: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = state.object(**id).unwrap();
            object.controller() == PlayerId(0)
                && object.card().supertypes.contains(&Supertype::Legendary)
        })
        .map(|id| state.name_of(*id).unwrap())
        .collect();
    found.sort();
    found
}

#[rstest]
#[case::no_aura(0, vec!["Alpha", "Beta"], 0)]
#[case::one_renamed(1, vec!["Beta", NEW_NAME], 0)]
#[case::two_legends_one_name(2, vec![NEW_NAME], 1)]
fn a_named_permanent_is_read_by_its_new_name(
    #[values(2, 3, 4)] seats: usize,
    #[case] auras: usize,
    #[case] kept: Vec<&str>,
    #[case] questions: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, auras).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, auras))?;
    let alpha = driver.id_of("Alpha")?;
    let beta = driver.id_of("Beta")?;
    // The legend rule asks first when two legends share the new name (CR 704.5j).
    let mut asked = 0;
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        asked += 1;
        assert!(asked <= 1, "one question for the legend rule");
        driver.answer_passively()?;
    }
    assert_eq!(asked, questions);
    if auras > 0 {
        let state = driver.state();
        // With two renamed legends the one kept is the controller's pick.
        let alpha = if state.object(alpha).unwrap().zone() == Zone::Battlefield {
            alpha
        } else {
            beta
        };
        assert_eq!(state.name_of(alpha).as_deref(), Some(NEW_NAME));
        let chars = characteristics(state, alpha).unwrap();
        assert!(chars.types.contains(CardType::Creature));
        assert!(chars.colors.contains(Color::Green) && chars.colors.contains(Color::White));
        assert!(!chars.colors.contains(Color::Red));
        assert!(chars.has_subtype(
            &state.object(alpha).unwrap().card().subtypes,
            &Subtype("Citizen".into())
        ));
        // Base 1/1, and the Admirer's +1/+1 finds it by its new name.
        assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
        assert!(chars.abilities.is_empty());
    } else {
        let chars = characteristics(driver.state(), alpha).unwrap();
        assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    }
    assert_eq!(legends(&driver), kept);
    replays(&driver, &setup);
    Ok(())
}
