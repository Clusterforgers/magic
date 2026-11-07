//! Goal: speed plays as the rules say at tables of two, three and four seats: start your engines! gives its controller speed 1, an opponent's life loss on that player's turn raises it once each turn up to 4, "your speed" counts it, "Max speed — " grants its ability only at 4, "each player who doesn't have max speed" spares the player who has it, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::keywords::keywords_of;
use mtg_engine::turn::Step;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{burn, cost, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A 2/2 with start your engines!, "Other creatures you control get +X/+0, where X is your speed." and "Max speed —
/// This creature has double strike."
fn racer() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Racer")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::StartYourEngines))
            .with_rules(ability(
                r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":{"Speed":"You"},"toughness":0}}]}}"#,
            ))
            .with_rules(ability(
                r#"{"Static":{"affected":"This","condition":{"MaxSpeed":"You"},"mods":[{"GrantAbility":{"Static":{"affected":"This","mods":[{"AddKeyword":"DoubleStrike"}]}}}]}}"#,
            )),
    )
}

/// An instant for `{R}`: 2 damage to each player who doesn't have max speed.
fn outpace() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Outpace")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(ability(
                r#"{"Spell":{"effects":[{"DealDamage":{"amount":2,"to":{"PlayersWhere":{"who":"EachPlayer","test":{"Not":{"MaxSpeed":"You"}}}}}}]}}"#,
            )),
    )
}

/// Seat 0 has the Racer, a Bear and twenty Mountains, and Blasts and an Outpace in hand.
fn table(seats: usize) -> Driver {
    let mut permanents = vec![ready(racer()), ready(free_creature("Grizzly Bears", 2, 2))];
    permanents.extend((0..20).map(|_| ready(mountain())));
    let mut hand: Vec<_> = (0..6).map(|n| burn(&format!("Blast {n}"), 1)).collect();
    hand.push(outpace());
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, permanents);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..20).map(|_| mountain()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Casts a Blast at `seat` and lets it resolve.
fn blast(driver: &mut Driver, name: &str, seat: usize) -> Result<(), ScriptError> {
    let target = driver.target_player(seat);
    driver.cast_at(name, &[target])?;
    resolve(driver)
}

/// Passes until the stack is empty.
fn resolve(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..50 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn speed(driver: &Driver) -> u8 {
    driver.state().speed(PlayerId(0))
}

fn bear_power(driver: &Driver) -> i32 {
    let id = driver.id_of("Grizzly Bears").unwrap();
    characteristics(driver.state(), id).unwrap().power.unwrap()
}

fn double_strike(driver: &Driver) -> bool {
    let id = driver.id_of("Racer").unwrap();
    keywords_of(driver.state(), id)
        .iter()
        .any(|held| held.name() == "Double strike")
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn speed_starts_rises_once_a_turn_and_grants_at_max(#[case] seats: usize) {
    let mut driver = table(seats);
    assert_eq!(speed(&driver), 1);
    assert_eq!(bear_power(&driver), 3);
    // Two losses in one turn raise it once; each later turn of seat 0 raises it once more.
    blast(&mut driver, "Blast 0", 1).unwrap();
    blast(&mut driver, "Blast 1", seats - 1).unwrap();
    assert_eq!(speed(&driver), 2);
    assert!(!double_strike(&driver));
    let mut turn = 2;
    for (n, was, expected) in [(2, 2, 3), (3, 3, 4), (4, 4, 4)] {
        turn += u32::try_from(seats).unwrap();
        driver.idle_until(turn, Step::Main1).unwrap();
        assert_eq!(speed(&driver), was);
        blast(&mut driver, &format!("Blast {n}"), 1).unwrap();
        assert_eq!(speed(&driver), expected);
    }
    assert_eq!(bear_power(&driver), 6);
    assert!(double_strike(&driver));
    // Seat 0 has max speed and is spared; every other seat takes 2.
    let before: Vec<i32> = driver.state().players().iter().map(|p| p.life()).collect();
    driver.cast("Outpace").unwrap();
    resolve(&mut driver).unwrap();
    let after: Vec<i32> = driver.state().players().iter().map(|p| p.life()).collect();
    assert_eq!(after[0], before[0]);
    assert!(
        after
            .iter()
            .zip(&before)
            .skip(1)
            .all(|(now, was)| *now == was - 2)
    );
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn without_max_speed_nothing_is_granted_and_everyone_takes_the_damage(#[case] seats: usize) {
    let mut driver = table(seats);
    assert!(!double_strike(&driver));
    let before: Vec<i32> = driver.state().players().iter().map(|p| p.life()).collect();
    driver.cast("Outpace").unwrap();
    resolve(&mut driver).unwrap();
    let after: Vec<i32> = driver.state().players().iter().map(|p| p.life()).collect();
    assert!(after.iter().zip(&before).all(|(now, was)| *now == was - 2));
    // Seat 1 lost life on seat 0's turn, so the speed rose; seat 0's own loss changed nothing more.
    assert_eq!(speed(&driver), 2);
}
