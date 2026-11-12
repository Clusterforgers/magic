//! Goal: "if it has dealt N or more damage to that player this turn" (`Amount::DamageDealtByTo`) counts only the damage this creature dealt this turn to the player of the event, never the damage it dealt to another player or another creature's damage to that player; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// A creature with "{0}: ~ deals 3 damage to each opponent.", "{0}: ~ deals 3 damage to you." and, when `vessel`,
/// "Whenever ~ deals damage to a player, if it has dealt 6 or more damage to that player this turn, they lose the game."
fn striker(name: &str, vessel: bool) -> Arc<CardDefinition> {
    let mut rules = vec![
        r#"{"Activated":{"cost":[],"effects":[{"DealDamage":{"to":"Opponents","amount":3}}]}}"#,
        r#"{"Activated":{"cost":[],"effects":[{"DealDamage":{"to":"You","amount":3}}]}}"#,
    ];
    if vessel {
        rules.push(
            r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer"}},"if":{"Compare":{"left":{"DamageDealtByTo":{"by":"This","to":"Triggering"}},"op":"Ge","right":6}},"effects":[{"LoseGame":{"who":"Triggering"}}]}}"#,
        );
    }
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1);
    for rule in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    Arc::new(card)
}

/// Lets every ability resolve; a game that ended stops it.
fn settle(driver: &mut Driver) {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return;
        }
        if driver.answer_passively().is_err() {
            return;
        }
    }
}

#[rstest]
#[case::once_at_opponents(&[("Vessel", 0)], false, false)]
#[case::twice_at_opponents(&[("Vessel", 0), ("Vessel", 0)], true, false)]
#[case::once_at_each(&[("Vessel", 0), ("Vessel", 1)], false, false)]
#[case::twice_at_you(&[("Vessel", 1), ("Vessel", 1)], false, true)]
#[case::another_creature_helps_not(&[("Helper", 0), ("Vessel", 0)], false, false)]
fn the_damage_counted_is_to_that_player_by_this_creature(
    #[case] uses: &[(&str, usize)],
    #[case] opponents_lose: bool,
    #[case] you_lose: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        [
            ready(striker("Vessel", true)),
            ready(striker("Helper", false)),
        ],
    ))
    .unwrap();
    for (name, ability) in uses {
        driver.begin_ability(name, *ability).unwrap();
        settle(&mut driver);
    }
    assert_eq!(!driver.state().is_alive(pid(0)), you_lose, "you");
    for seat in 1..seats {
        assert_eq!(
            !driver.state().is_alive(pid(seat)),
            opponents_lose,
            "seat {seat}"
        );
    }
}
