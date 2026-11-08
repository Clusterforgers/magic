//! Goal: "destroy up to three other target nonland permanents; for each of those permanents, its controller creates a
//! 3/3 Angel" (Soul of Emancipation) counts the targets, destroyed or not, at tables of two to four seats: each target's
//! controller gets one Angel, an indestructible target still counts, and no target gives no Angel.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The trigger as the translator writes Soul of Emancipation's.
const SOUL: &str = r#"{"Triggered":{"effects":[{"Destroy":{"what":{"Target":0}}},{"Together":{"effects":[{"RepeatEach":{"as":"each","effects":[{"CreateToken":{"token":{"Custom":{"abilities":[{"Keyword":"Flying"}],"colors":["White"],"name":"Angel","power":3,"subtypes":["Angel"],"toughness":3,"types":["Creature"]}},"who":{"ControllerOf":{"Bound":"each"}}}}],"over":{"Target":0}}}]}}],"targets":[{"bounds":{"max":3,"min":0},"what":{"Object":[{"Not":{"Type":"Land"}},"Other"]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;

fn soul() -> Arc<CardDefinition> {
    let card = free_creature("Soul", 5, 7);
    let mut card = (*card).clone();
    card = card.with_rules(serde_json::from_str(SOUL).unwrap());
    Arc::new(card)
}

/// The number of Angels each seat controls.
fn angels(driver: &Driver, seats: usize) -> Vec<usize> {
    (0..seats)
        .map(|seat| {
            driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .filter(|id| {
                    let object = driver.state().object(**id).unwrap();
                    object.card().name == "Angel" && object.controller() == pid(seat)
                })
                .count()
        })
        .collect()
}

#[rstest]
fn each_target_gives_its_controller_an_angel(
    #[values(2, 3, 4)] seats: usize,
    #[values(vec![], vec!["Wall"], vec!["Bear", "Wall"], vec!["Bear", "Wall", "Mine"])] aims: Vec<
        &str,
    >,
) -> Result<(), ScriptError> {
    let other = seats - 1;
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [soul()])
            .battlefield(0, vec![ready(free_creature("Mine", 1, 1))])
            .battlefield(
                other,
                vec![
                    ready(free_creature("Bear", 2, 2)),
                    ready(keyworded("Wall", 0, 4, &[Keyword::Indestructible])),
                ],
            ),
    )?;
    driver.cast("Soul")?;
    let targets: Vec<Target> = aims
        .iter()
        .map(|name| driver.target_named(name))
        .collect::<Result<_, _>>()?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
            && driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .any(|id| driver.state().object(*id).unwrap().card().name == "Soul")
        {
            break;
        }
        let answer = match &decision.kind {
            DecisionKind::ChooseTargets(_) => {
                Answer::ChooseTargets(TargetChoice::Targets(targets.clone()))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    let mut expected = vec![0; seats];
    expected[other] = aims.iter().filter(|name| **name != "Mine").count();
    expected[0] = usize::from(aims.contains(&"Mine"));
    assert_eq!(angels(&driver, seats), expected, "{aims:?}");
    Ok(())
}
