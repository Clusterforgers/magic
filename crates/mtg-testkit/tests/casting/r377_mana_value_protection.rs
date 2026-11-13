//! Goal: "artifacts you control have protection from each mana value among artifacts you control" (Rebbec) keeps a
//! spell of a mana value one of those artifacts has from targeting them and lets one of another mana value, and the
//! protection follows the artifacts there are, at two to four seats; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::setup::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// An artifact of the mana value.
fn relic(name: &str, value: u32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(value, &[])),
    )
}

/// A sorcery of the mana value that destroys target artifact.
fn shatter(value: u32) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Shatter")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(value, &[]))
            .with_rules(rule),
    )
}

#[rstest]
fn r377_rebbec_protects_from_the_mana_values_of_its_artifacts(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4)] spell: u32,
    #[values(false, true)] rebbec: bool,
) {
    let mut mine = vec![ready(relic("Relic Two", 2)), ready(relic("Relic Three", 3))];
    if rebbec {
        mine.push(ready(card("Rebbec, Architect of Ascension").unwrap()));
    }
    let lands: Vec<PresetPermanent> = (0..4).map(|_| ready(mountain())).collect();
    let scenario = Scenario::new(seats)
        .at(3, 1, Step::Main1)
        .battlefield(0, mine)
        .battlefield(1, lands)
        .hand(1, [shatter(spell)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let at = driver.target_named("Relic Two").unwrap();
    let cast = driver.cast_at("Shatter", &[at]);
    let protected = rebbec && (spell == 2 || spell == 3);
    assert_eq!(cast.is_err(), protected, "a spell of mana value {spell}");
    if !protected {
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
        assert!(driver.target_named("Relic Two").is_err(), "destroyed");
        let setup = scenario.setup().unwrap();
        let replayed = replay(&setup, driver.engine().events()).unwrap();
        assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    }
}
