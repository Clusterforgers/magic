//! Goal: "~ has haste and ward {10} as long as it was cast" (The Tarrasque) gives the keywords to the permanent a cast spell became, and not to the same card put onto the battlefield without being cast, at tables of two, three and four seats; every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::c4_control::{replays, settle};

const TARRASQUE: &str = "The Tarrasque";

fn keywords(driver: &Driver) -> (bool, bool) {
    let id = driver.id_of(TARRASQUE).unwrap();
    let ward = Keyword::Ward(Cost::mana("{10}".parse().unwrap()));
    (
        has_keyword(driver.state(), id, &Keyword::Haste),
        has_keyword(driver.state(), id, &ward),
    )
}

#[rstest]
fn a_cast_tarrasque_has_haste_and_ward(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(TARRASQUE).unwrap()])
        .battlefield(0, (0..9).map(|_| ready(forest())));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(TARRASQUE)?;
    settle(&mut driver, None)?;
    assert_eq!(keywords(&driver), (true, true));
    replays(&driver);
    Ok(())
}

#[rstest]
fn a_tarrasque_put_onto_the_battlefield_has_neither(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(TARRASQUE).unwrap())]);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(keywords(&driver), (false, false));
    replays(&driver);
    Ok(())
}
