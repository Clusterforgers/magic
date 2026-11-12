//! Goal: "At the beginning of combat on your turn, choose flying or indestructible at random. ~ gains that ability until end of turn." (Blitzwing, Adaptive Assailant) gives its source exactly one of the two keywords at each beginning of combat of its controller, both turning up across seeds, and none after the turn ends, at 2 to 4 seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::replays;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_testkit::trigger_cards::sized_creature_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const TRIGGER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"Modal":{"at_random":true,"modes":[{"effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]},{"effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}]}}]}}"#;

fn table(seats: usize, seed: u64) -> Scenario {
    let ability: Ability = serde_json::from_str(TRIGGER).unwrap();
    Scenario::new(seats).seed(seed).battlefield(
        0,
        [ready(sized_creature_with("Shifter", 3, 3, vec![ability]))],
    )
}

/// Which of flying and indestructible the Shifter has.
fn gained(driver: &Driver) -> (bool, bool) {
    let id = driver.id_of("Shifter").unwrap();
    let keywords = keywords_of(driver.state(), id);
    (
        keywords.contains(&&Keyword::Flying),
        keywords.contains(&&Keyword::Indestructible),
    )
}

#[rstest]
fn one_keyword_at_random_for_the_turn(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut seen = (false, false);
    for seed in 0..12 {
        let setup = table(seats, seed).setup().unwrap();
        let mut driver = Driver::from_scenario(table(seats, seed))?;
        assert_eq!(gained(&driver), (false, false));
        driver.pass_until(Step::DeclareAttackers)?;
        let (flying, indestructible) = gained(&driver);
        assert!(
            flying != indestructible,
            "exactly one keyword: {flying} {indestructible}"
        );
        seen.0 |= flying;
        seen.1 |= indestructible;
        replays(&driver, &setup);
        driver.idle_until(2, Step::Upkeep)?;
        assert_eq!(gained(&driver), (false, false), "until end of turn");
    }
    assert_eq!(seen, (true, true), "both keywords turn up");
    Ok(())
}
