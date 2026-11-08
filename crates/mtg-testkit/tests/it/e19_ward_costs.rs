//! Goal: ward takes any cost the engine pays ("Ward—Blight 2", "Ward—Pay 2 life"): the player whose ability targets the
//! warded permanent pays it and the ability resolves, or declines and the ability is countered, at tables of two to four
//! seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::creature;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// A 4/4 with ward of this cost.
fn warded(cost: &str) -> Arc<CardDefinition> {
    let rule = format!(r#"{{"Keyword":{{"Ward":{cost}}}}}"#);
    Arc::new(
        CardDefinition::new("Warded")
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// "{T}: Tap target creature."
fn tapper() -> Arc<CardDefinition> {
    let rule = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#;
    Arc::new(
        CardDefinition::new("Tapper")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(rule).unwrap()),
    )
}

fn object(driver: &Driver, name: &str) -> mtg_core::ids::ObjectId {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap()
}

#[rstest]
#[case::blight_paid(r#"[{"Blight":2}]"#, true)]
#[case::blight_declined(r#"[{"Blight":2}]"#, false)]
#[case::life_paid(r#"[{"PayLife":2}]"#, true)]
#[case::life_declined(r#"[{"PayLife":2}]"#, false)]
fn ward_of_any_cost_is_paid_or_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] cost: &str,
    #[case] pays: bool,
) {
    let scenario = Scenario::new(seats)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(warded(cost))])
        .battlefield(1, [ready(tapper()), ready(creature("Bear", 0, &[], 3, 3))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Tapper").unwrap();
    let target = driver.target_named("Warded").unwrap();
    driver.choose_targets(&[target]).unwrap();
    let mut asked = false;
    for _ in 0..40 {
        if asked && driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) => {
                assert_eq!(decision.player, pid(1), "the targeting player pays");
                asked = true;
                Answer::YesNo(YesNoChoice(pays))
            }
            DecisionKind::ChooseCards(options) => {
                let picked = options.from.iter().copied().take(options.min as usize);
                Answer::ChooseCards(CardsChoice(picked.collect()))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    assert!(asked, "ward asks for its cost");
    let state = driver.state();
    let warded = object(&driver, "Warded");
    assert_eq!(
        state.object(warded).unwrap().permanent().unwrap().tapped(),
        pays,
        "the ability resolves only when the cost is paid"
    );
    let bear = object(&driver, "Bear");
    let blighted = state
        .object(bear)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::MinusOneMinusOne);
    let life = state.player(pid(1)).unwrap().life();
    let start = state.player(pid(0)).unwrap().life();
    match (cost.contains("Blight"), pays) {
        (true, true) => assert_eq!(blighted, 2),
        (false, true) => assert_eq!(life, start - 2),
        _ => assert_eq!((blighted, life), (0, start)),
    }
}
