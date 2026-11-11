//! Goal: the engine pieces of ep33 play by the rules at tables of two and three seats. A card an effect returns to the battlefield face down (`Destination::BattlefieldFaceDown`) is a 2/2 creature with no name under the player the move names, only that player looks at it, and it is turned face up for its morph cost and never for its mana cost; a face-down card that has no morph stays face down.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{DecisionKind, PriorityAction};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::FaceDown;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::cards::{cost, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A 4/1 for {2} that returns face down under `under` when it dies, with morph {1} when `morph`, and "Sacrifice ~: You
/// gain 1 life."
fn phoenix(morph: bool, under: &str) -> Arc<CardDefinition> {
    let rule = |json: &str| -> Ability { serde_json::from_str(json).unwrap() };
    let mut card = CardDefinition::new("Phoenix")
        .with_type(CardType::Creature)
        .with_mana_cost(cost(2, &[]))
        .with_power_toughness(4, 1)
        .with_rules(rule(&format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"Move":{{"what":"This","to":{{"BattlefieldFaceDown":{under}}}}}}}]}}}}"#
        )))
        .with_rules(rule(
            r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#,
        ));
    if morph {
        card = card.with_rules(rule(r#"{"Keyword":{"Morph":"{1}"}}"#));
    }
    Arc::new(card)
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// The face-down permanents on the battlefield.
fn face_down(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .collect()
}

/// The permanents seat 0 may turn face up now.
fn turnable(driver: &Driver) -> Vec<ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.turnable.clone(),
        _ => Vec::new(),
    }
}

#[rstest]
fn a_card_returned_face_down_is_a_hidden_2_2_turned_up_only_for_its_morph(
    #[values(2, 3)] seats: usize,
    #[values(true, false)] morph: bool,
    #[values(r#"{"under":"You"}"#, "{}")] under: &str,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        vec![
            ready(phoenix(morph, under)),
            ready(plains()),
            ready(plains()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Phoenix")?;
    settle(&mut driver)?;
    let made = face_down(&driver);
    assert_eq!(made.len(), 1, "the card came back face down");
    let id = made[0];
    let state = driver.state();
    let object = state.object(id).unwrap();
    assert_eq!(object.face_down(), Some(FaceDown::Morph));
    assert_eq!(object.whole_card().name, "Phoenix");
    assert_eq!(object.card().name, "");
    assert_eq!(object.controller(), pid(0));
    let chars = characteristics(state, id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
    assert!(chars.types.contains(CardType::Creature));
    for seat in 0..seats {
        assert_eq!(state.may_look_at(pid(seat), id), seat == 0, "seat {seat}");
    }
    let cause = driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::ZoneChanged {
                object,
                to: Zone::Battlefield,
                cause,
                ..
            } if *object == id => Some(*cause),
            _ => None,
        });
    assert_eq!(cause, Some(ZoneCause::PutFaceDown { by: pid(0) }));
    // Its mana cost of {2} is paid by the two Plains, yet only morph turns it face up.
    assert_eq!(turnable(&driver), if morph { vec![id] } else { vec![] });
    if morph {
        driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
        crate::common::casting::pay(&mut driver)?;
        let object = driver.state().object(id).unwrap();
        assert_eq!(object.face_down(), None);
        assert_eq!(object.card().name, "Phoenix");
    }
    Ok(())
}
