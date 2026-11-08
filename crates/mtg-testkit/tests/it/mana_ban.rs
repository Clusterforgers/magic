//! Goal: a prohibition of every activated ability stops the mana abilities of the permanents it names, one of non-mana abilities leaves them, and a ban on other players' permanents leaves the caster's own.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Whether seat 0 casts a one-mana creature with one Forest while the Warden's rule is in play.
fn cub_cast(who: &str, kind: &str, sources: &str) -> bool {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Rule":{{"rule":{{"Cant":{{"who":{who},"action":{{"ActivateKind":{{"kind":"{kind}","sources":{sources}}}}}}}}}}}}}"#
    ))
    .unwrap();
    let warden = Arc::new(
        CardDefinition::new("Warden")
            .with_type(CardType::Artifact)
            .with_rules(rule),
    );
    let scenario = Scenario::new(2)
        .at(2, 0, Step::Main1)
        .hand(0, vec![creature("Cub", 1, &[], 1, 1)])
        .battlefield(0, vec![ready(warden), ready(forest())])
        .library(0, (0..10).map(|_| forest()))
        .library(1, (0..10).map(|_| forest()));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let cast = driver.cast("Cub").is_ok();
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    cast
}

#[rstest]
#[case::every_ability_of_lands(r#""EachPlayer""#, "Every", r#"[{"Type":"Land"}]"#, false)]
#[case::every_ability_of_all(r#""EachPlayer""#, "Every", "[]", false)]
#[case::every_ability_of_artifacts_only(
    r#""EachPlayer""#,
    "Every",
    r#"[{"Type":"Artifact"}]"#,
    true
)]
#[case::non_mana_abilities_of_lands(r#""EachPlayer""#, "Any", r#"[{"Type":"Land"}]"#, true)]
#[case::opponents_activating(r#""Opponents""#, "Every", r#"[{"Type":"Land"}]"#, true)]
#[case::you_activating(r#""You""#, "Every", r#"[{"Type":"Land"}]"#, false)]
fn the_ban_on_every_ability_covers_mana_abilities(
    #[case] who: &str,
    #[case] kind: &str,
    #[case] sources: &str,
    #[case] cast: bool,
) {
    assert_eq!(cub_cast(who, kind, sources), cast);
}
