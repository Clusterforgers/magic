//! Goal: on the wire the refusals told in detail read back as sent: a missing host names how the spell would go on it, an X with no target its range, a rewind of an activation says `activated` and a cast's leaves it out, and at two to four seats the holder's offer carries why an ability of a graveyard card cannot be activated.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::Event;
use mtg_engine::decision::{DecisionKind, HostBy, TargetZone, Unplayable, Why};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::island;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;
use serde_json::{Value, json};

#[rstest]
#[case::an_aura(Why::Host { needs: "a creature on the battlefield".to_owned(), by: HostBy::Enchant }, json!({"Host": {"needs": "a creature on the battlefield", "by": "Enchant"}}))]
#[case::a_bestow(Why::Host { needs: "a creature on the battlefield".to_owned(), by: HostBy::Bestow }, json!({"Host": {"needs": "a creature on the battlefield", "by": "Bestow"}}))]
#[case::a_mutate(Why::Host { needs: "a non-Human creature you own on the battlefield".to_owned(), by: HostBy::Mutate }, json!({"Host": {"needs": "a non-Human creature you own on the battlefield", "by": "Mutate"}}))]
#[case::an_x(Why::XTarget { needs: "a creature card with mana value X in your graveyard".to_owned(), zone: TargetZone::Graveyard, least: 0, most: 3 }, json!({"XTarget": {"needs": "a creature card with mana value X in your graveyard", "zone": "Graveyard", "least": 0, "most": 3}}))]
fn a_detail_reads_back_as_sent(#[case] why: Why, #[case] wire: Value) {
    assert_eq!(serde_json::to_value(&why).ok(), Some(wire.clone()));
    assert_eq!(serde_json::from_value::<Why>(wire).ok(), Some(why));
}

#[rstest]
fn a_rewind_says_activated_only_for_an_activation(#[values(false, true)] activated: bool) {
    let event = Event::AnnouncementRewound {
        player: PlayerId(0),
        object: ObjectId(7),
        reason: Unplayable::Mana,
        why: None,
        activated,
    };
    let wire = serde_json::to_value(&event).unwrap_or(Value::Null);
    assert_eq!(
        wire["AnnouncementRewound"].get("activated").is_some(),
        activated
    );
    assert_eq!(serde_json::from_value::<Event>(wire).ok(), Some(event));
}

#[rstest]
fn the_holder_reads_why_a_graveyard_ability_waits(#[values(2, 3, 4)] players: usize) {
    let zombie = mtg_testkit::corpus::card("Dregscape Zombie").unwrap();
    let mut scenario = Scenario::new(players)
        .at(2, 0, Step::Main1)
        .graveyard(0, [zombie]);
    for seat in 0..players {
        scenario = scenario.library(seat, (0..5).map(|_| island()));
    }
    let driver = Driver::from_scenario(scenario).unwrap();
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, PlayerId(0));
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("a priority decision");
    };
    let card = driver.state().zone(Zone::Graveyard(PlayerId(0)))[0];
    let wire = serde_json::to_value(options.as_ref()).unwrap_or(Value::Null);
    let entry = wire["inactive"]
        .as_array()
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry["ability"]["object"] == json!(card.0))
        })
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(entry["reason"], json!("Mana"));
    assert_eq!(entry["why"]["Mana"]["at_hand"], json!(0));
    assert_eq!(entry["why"]["Mana"]["missing"], json!(["Black"]));
}
