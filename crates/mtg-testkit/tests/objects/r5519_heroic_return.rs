//! Goal: Heroic Return returns the target creature card from its caster's graveyard to the battlefield, with two
//! +1/+1 counters exactly when it is a Hero, at two to four seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::Subtype;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A 2/2 creature card named `name` with one creature type.
fn r5519_typed(name: &str, subtype: &str) -> Arc<CardDefinition> {
    let mut made = (*free_creature(name, 2, 2)).clone();
    made.subtypes = vec![Subtype(subtype.into())];
    Arc::new(made)
}

#[rstest]
fn r5519_heroic_return_counts_heroes_only(
    #[values(2, 3, 4)] seats: usize,
    #[values("Hero", "Soldier")] subtype: &str,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Heroic Return").unwrap()])
        .graveyard(0, [r5519_typed("Fallen", subtype)])
        .battlefield(0, (0..6).map(|_| ready(plains())).collect::<Vec<_>>());
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let fallen = driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(0)))
        .iter()
        .find_map(|id| driver.state().object(*id))
        .map(|object| Target::Object(object.reference()))
        .unwrap();
    driver.cast_at("Heroic Return", &[fallen]).unwrap();
    for _ in 0..20 {
        if driver.state().stack().is_empty()
            || !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
        {
            break;
        }
        driver.pass().unwrap();
    }
    let id = driver.id_of("Fallen").unwrap();
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.zone(), Zone::Battlefield);
    let counters = object
        .permanent()
        .unwrap()
        .counters()
        .map(|(_, count)| count)
        .sum::<u32>();
    assert_eq!(counters, if subtype == "Hero" { 2 } else { 0 });
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
