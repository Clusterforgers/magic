//! Goal: an ability on the stack that a player who stays controls outlives its source when the source's owner leaves the game (CR 800.4a, 113.7a), at tables of three and four seats: a stolen pinger's ability resolves after its owner conceded, its damage comes from the source as it last existed and reaches each target, the invariants hold through it and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::copying::replays;
use crate::common::pid;

use mtg_cards::keyword::Keyword;
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_testkit::cards::pinger;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

#[rstest]
fn a_stolen_pingers_ability_resolves_after_its_owner_left(
    #[values(3, 4)] seats: usize,
    #[values(0, 2)] target: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(1, [ready(pinger("Pinger", 0))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let source = driver.id_of("Pinger")?;
    driver.add_effect(
        driver.on(source),
        Modification::SetController(pid(0)),
        Duration::Forever,
    )?;
    driver.add_effect(
        driver.on(source),
        Modification::AddKeyword(Keyword::Haste),
        Duration::Forever,
    )?;
    driver.begin_activation("Pinger")?;
    let aim = driver.target_player(target);
    driver.choose_targets(&[aim])?;
    let life = driver.state().player(pid(target)).unwrap().life();
    driver.concede(pid(1))?;
    assert!(
        driver.state().object(source).is_none(),
        "the Pinger left the game with its owner"
    );
    assert_eq!(
        driver.state().stack().len(),
        1,
        "its ability stays on the stack"
    );
    let mut dealt = Vec::new();
    while !driver.state().stack().is_empty() {
        dealt.extend(
            driver
                .pass()?
                .into_iter()
                .filter(|e| matches!(e, Event::DamageDealt { .. })),
        );
    }
    assert!(
        matches!(dealt.as_slice(), [Event::DamageDealt { source: s, amount: 1, .. }] if *s == source),
        "{dealt:?}"
    );
    assert_eq!(driver.state().player(pid(target)).unwrap().life(), life - 1);
    replays(&driver, &setup);
    Ok(())
}
