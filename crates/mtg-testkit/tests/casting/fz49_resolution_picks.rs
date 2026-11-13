//! Goal: a cost paid while an ability resolves that takes cards for their total power or mana value (crew-like taps,
//! collect evidence) refuses a pick short of the total as an illegal answer that changes nothing, as the same pick of an
//! announced cost is refused, and takes a pick that has enough.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::error::{EngineError, IllegalAnswer};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use common::pid;

/// An artifact with "{0}: You may pay <cost>. If you do, you gain 1 life."
fn gadget(cost: &str) -> Arc<CardDefinition> {
    let ability = format!(
        r#"{{"Activated":{{"cost":[],"effects":[{{"Pay":{{"cost":[{cost}],"paid":[{{"GainLife":{{"amount":1}}}}]}}}}]}}}}"#
    );
    Arc::new(
        CardDefinition::new("Gadget")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&ability).unwrap()),
    )
}

fn object(driver: &Driver, name: &str) -> ObjectId {
    driver
        .state()
        .objects()
        .find(|found| found.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap()
}

/// A Gadget whose cost needs 4, and the cards "Small" (1), "Mid" (2) and "Big" (3), of which "Big" alone falls short.
fn table(evidence: bool) -> Scenario {
    let small = creature("Small", 1, &[], 1, 1);
    let big = creature("Big", 3, &[], 3, 3);
    let mid = creature("Mid", 2, &[], 2, 2);
    if evidence {
        Scenario::new(2)
            .battlefield(0, [ready(gadget(r#"{"CollectEvidence":4}"#))])
            .graveyard(0, vec![small, mid, big])
    } else {
        let tap =
            r#"{"TapPower":{"filter":[{"Type":"Creature"},{"ControlledBy":"You"}],"power":4}}"#;
        Scenario::new(2).battlefield(
            0,
            [ready(gadget(tap)), ready(small), ready(mid), ready(big)],
        )
    }
}

#[rstest]
#[case::collect_evidence(true)]
#[case::tap_for_total_power(false)]
fn a_short_pick_of_a_cost_paid_on_resolution_is_refused(
    #[case] evidence: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(evidence))?;
    driver.begin_activation("Gadget")?;
    for _ in 0..20 {
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    driver.answer(Answer::YesNo(YesNoChoice(true)))?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseCards(_)
    ));
    let before = driver.state().state_hash();
    let error = driver
        .answer(Answer::ChooseCards(CardsChoice(vec![object(
            &driver, "Big",
        )])))
        .unwrap_err();
    assert!(
        matches!(
            error,
            ScriptError::Engine(EngineError::Illegal(
                IllegalAnswer::TooLittleValue { needed: 4, got: 3 }
                    | IllegalAnswer::TooLittlePower { needed: 4, got: 3 }
            ))
        ),
        "{error:?}"
    );
    assert_eq!(
        driver.state().state_hash(),
        before,
        "a refusal changes nothing"
    );
    let both = vec![object(&driver, "Small"), object(&driver, "Big")];
    driver.answer(Answer::ChooseCards(CardsChoice(both)))?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 21);
    if evidence {
        assert_eq!(driver.state().zone(Zone::Exile).len(), 2);
    }
    Ok(())
}
