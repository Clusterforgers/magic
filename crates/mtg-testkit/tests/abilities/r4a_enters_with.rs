//! Goal: a permanent that enters with counters and with a keyword or an ability (CR 614.1c, a kicked Kavu Titan) has both as soon as it is on the battlefield, at tables of two to four seats, and a card that enters without the replacement has neither.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use mtg_engine::keywords::keywords_of;

fn titan(extra: &str) -> Arc<CardDefinition> {
    let enters = format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"WithCountersAnd":{{"counter":"+1/+1","count":3,"mods":[{extra}]}}}}}}}}"#
    );
    let card = (*free_creature("Titan", 2, 2)).clone();
    Arc::new(card.with_rules(serde_json::from_str::<Ability>(&enters).unwrap()))
}

fn bear() -> Arc<CardDefinition> {
    free_creature("Bear", 2, 2)
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .filter(|(kind, _)| **kind == CounterKind::PlusOnePlusOne)
        .map(|(_, n)| n)
        .sum()
}

fn play(seats: usize, name: &str, card: Arc<CardDefinition>) -> Driver {
    let mut scenario = Scenario::new(seats).hand(0, vec![card]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..16).map(|_| plains()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast(name).unwrap();
    for _ in 0..12 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass().unwrap();
    }
    driver
}

#[rstest]
fn it_enters_with_the_counters_and_the_keyword(#[values(2, 3, 4)] seats: usize) {
    let driver = play(seats, "Titan", titan(r#"{"AddKeyword":"Trample"}"#));
    assert_eq!(counters(&driver, "Titan"), 3);
    let id = driver.id_of("Titan").unwrap();
    assert!(keywords_of(driver.state(), id).contains(&&Keyword::Trample));
}

#[rstest]
fn it_enters_with_a_granted_ability(#[values(2, 3)] seats: usize) {
    let granted = r#"{"GrantAbility":{"Activated":{"cost":[{"PayLife":3}],"effects":[{"Regenerate":{"what":"This"}}]}}}"#;
    let driver = play(seats, "Titan", titan(granted));
    assert_eq!(counters(&driver, "Titan"), 3);
}

#[test]
fn a_plain_creature_has_no_counters_or_keyword() {
    let driver = play(2, "Bear", bear());
    assert_eq!(counters(&driver, "Bear"), 0);
    let id = driver.id_of("Bear").unwrap();
    assert!(!keywords_of(driver.state(), id).contains(&&Keyword::Trample));
}
