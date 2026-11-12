//! Goal: "if it would leave the battlefield, exile it instead" (Kheru Lich Lord, Gruesome Encore, Whip of Erebos) exiles the named permanent when it would be returned to its owner's hand or destroyed, and lets the end-step exile happen, while without the shield each move goes where it says (CR 614.1a, 614.6); at tables of two to four seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::trigger_cards::creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// "{0}: If this would leave the battlefield, exile it instead; at the beginning of your end step, exile it",
/// "{0}: Return this to its owner's hand", "{0}: Destroy this".
fn fix19_lich() -> Arc<CardDefinition> {
    let rules = [
        r#"{"Activated":{"cost":[],"effects":[{"Shield":{"by":{"MoveTo":"Exile"},"duration":"Forever","when":{"ZoneChange":{"from":"Battlefield","what":[{"Is":"This"}]}}}},{"DelayedTrigger":{"effects":[{"Exile":{"what":"This"}}],"when":{"StepBegins":{"step":"End","whose":"You"}}}}]}}"#,
        r#"{"Activated":{"cost":[],"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#,
        r#"{"Activated":{"cost":[],"effects":[{"Destroy":{"what":"This"}}]}}"#,
    ];
    creature_with(
        "Lich",
        rules
            .iter()
            .map(|rule| serde_json::from_str(rule).unwrap())
            .collect(),
    )
}

/// What happens to the Lich after its first ability, if any.
#[derive(Debug, Clone, Copy)]
enum Move {
    Bounce,
    Destroy,
    EndStep,
}

fn fix19_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn fix19_a_shielded_permanent_is_exiled_whichever_way_it_leaves(
    #[values(2, 3, 4)] seats: usize,
    #[values(Move::Bounce, Move::Destroy, Move::EndStep)] leaving: Move,
    #[values(true, false)] shielded: bool,
) -> Result<(), ScriptError> {
    let scenario = || Scenario::new(seats).battlefield(0, [ready(fix19_lich())]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.pass_until(Step::Main1)?;
    if shielded {
        driver.begin_ability("Lich", 0)?;
        fix19_settle(&mut driver)?;
    }
    match leaving {
        Move::Bounce => driver.begin_ability("Lich", 1).map(drop)?,
        Move::Destroy => driver.begin_ability("Lich", 2).map(drop)?,
        Move::EndStep => {
            let turn = driver.state().turn().number();
            driver.idle_until(turn, Step::End)?;
        }
    }
    fix19_settle(&mut driver)?;
    let expected = match (shielded, leaving) {
        (true, _) => Zone::Exile,
        (false, Move::Bounce) => Zone::Hand(pid(0)),
        (false, Move::Destroy) => Zone::Graveyard(pid(0)),
        (false, Move::EndStep) => Zone::Battlefield,
    };
    assert_eq!(names(driver.state(), expected), ["Lich"], "{leaving:?}");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
