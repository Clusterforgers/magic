//! Goal: a permission to cast from a graveyard reaches another player's graveyard only when it names the owner of its
//! cards ("from an opponent's graveyard"); one written "from your graveyard" stays with its controller's own; a card
//! cast from another player's graveyard with "exile it instead" ends in exile; and the games replay from their logs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{instant, island};
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::common::pid;

const CASTER: usize = 0;
const OTHER: usize = 1;

/// An emblem that lets its owner cast instants of `owner` (none: no owner named) from a graveyard free, once a turn,
/// exiled after.
fn permission(owner: Option<&str>) -> Arc<CardDefinition> {
    let named = owner.map_or(String::new(), |o| format!(r#",{{"OwnedBy":"{o}"}}"#));
    let rule = format!(
        r#"{{"Rule":{{"rule":{{"MayCast":{{"from":"Graveyard","free":true,"once":true,"riders":["ExiledInstead"],"what":[{{"Type":"Instant"}}{named}]}}}}}}}}"#
    );
    let ability: Ability = serde_json::from_str(&rule).unwrap();
    Arc::new(emblem_definition("Permission", &[ability]))
}

#[rstest]
#[case::opponents_reach_the_other_graveyard(Some("Opponents"), OTHER, true)]
#[case::opponents_skip_the_own_graveyard(Some("Opponents"), CASTER, false)]
#[case::any_owner_reaches_the_other_graveyard(Some("EachPlayer"), OTHER, true)]
#[case::your_graveyard_stays_at_home(None, OTHER, false)]
#[case::your_graveyard_casts_from_your_own(None, CASTER, true)]
fn a_permission_reaches_another_graveyard_only_when_it_names_the_owner(
    #[case] owner: Option<&str>,
    #[case] holder: usize,
    #[case] castable: bool,
) {
    let scenario = Scenario::new(2)
        .library(CASTER, (0..10).map(|_| island()))
        .library(OTHER, (0..10).map(|_| island()))
        .graveyard(holder, [instant("Lesson", 2, &[])])
        .emblems(CASTER, [permission(owner)])
        .at(1, CASTER, Step::Main1);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(driver.cast("Lesson").is_ok(), castable);
    if castable {
        driver.pass().unwrap();
        driver.pass().unwrap();
        let state = driver.state();
        assert_eq!(state.zone(Zone::Exile).len(), 1);
        assert_eq!(state.zone(Zone::Graveyard(pid(holder))), []);
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
