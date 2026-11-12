//! Goal: "defending player" read by a permanent that is not attacking names the defending players of the combat (CR 506.2, 802.2), on the attacking and the defending side, so a restriction on what the defending player controls applies (Arcum's Sleigh, Kjeldoran Guard); and a turn record judges its filter on the subject as it was when the record was made (CR 608.2h), so +1/+1 counters put on a creature you controlled still count after it died, was bounced, or changed control, and counters put on an opponent's creature you later take do not (Iridescent Hornbeetle); at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Supertype};
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{on_battlefield, spell};

/// An artifact with "{0}: you gain 1 life. Activate only during combat and only if defending player controls
/// `op` `right` snow lands", the restriction the translator writes for Arcum's Sleigh and Kjeldoran Guard.
fn sleigh(op: &str, right: u32) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":[],"condition":{{"All":[{{"Compare":{{"left":{{"Count":[{{"Type":"Land"}},{{"Supertype":"Snow"}},{{"ControlledBy":"Defending"}}]}},"op":"{op}","right":{right}}}}},"InCombat"]}},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Sleigh")
            .with_type(CardType::Artifact)
            .with_rules(rules),
    )
}

fn snow_land() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Snowfield")
            .with_type(CardType::Land)
            .with_supertype(Supertype::Snow),
    )
}

/// Seat 0 attacks seat `attacked` with a Bear; the Sleigh sits at seat `owner`, a snow land at seat `snow`. Whether
/// the owner may activate the Sleigh when it next gets priority in the declare-attackers step.
fn offered_in_combat(
    seats: usize,
    op: &str,
    (owner, snow, attacked): (usize, usize, usize),
) -> Result<bool, ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::DeclareAttackers);
    for seat in 0..seats {
        let mut permanents = Vec::new();
        if seat == 0 {
            permanents.push(ready(free_creature("Bear", 2, 2)));
        }
        if seat == owner {
            permanents.push(ready(sleigh(op, u32::from(op == "Ge"))));
        }
        if seat == snow {
            permanents.push(ready(snow_land()));
        }
        scenario = scenario
            .battlefield(seat, permanents)
            .library(seat, (0..8).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Bear", attacked)])?;
    for _ in 0..seats {
        let deciding = driver.decision()?.player.index();
        if deciding == owner {
            return Ok(!driver.offered_abilities("Sleigh")?.is_empty());
        }
        driver.pass()?;
    }
    panic!("the owner never got priority");
}

#[rstest]
// "Defending player controls a snow land" (Arcum's Sleigh).
#[case::attacker_side_defender_has_snow("Ge", (0, 1, 1), true)]
#[case::attacker_side_nobody_has_snow("Ge", (0, 0, 1), false)]
#[case::defender_side_has_snow("Ge", (1, 1, 1), true)]
// "Defending player controls no snow lands" (Kjeldoran Guard).
#[case::guard_defender_has_snow("Eq", (0, 1, 1), false)]
#[case::guard_nobody_has_snow("Eq", (0, 0, 1), true)]
#[case::guard_on_defender_side_with_snow("Eq", (1, 1, 1), false)]
fn defending_player_outside_the_attack_is_the_combats(
    #[values(2, 3, 4)] seats: usize,
    #[case] op: &str,
    #[case] seats_of: (usize, usize, usize),
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    assert_eq!(offered_in_combat(seats, op, seats_of)?, offered);
    Ok(())
}

#[rstest]
fn defending_player_is_nobody_outside_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(sleigh("Eq", 0))])
            .battlefield(1, [ready(snow_land())]),
    )?;
    assert_eq!(driver.offered_abilities("Sleigh")?, Vec::<usize>::new());
    Ok(())
}

/// Passes priority until the stack is empty and nothing waits to go on it.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..24 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if priority
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        if priority {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    panic!("the stack did not empty");
}

/// A 1/1 that gets +2/+0 while you put a +1/+1 counter on a creature you controlled this turn.
fn watcher() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Static":{"affected":"This","condition":{"Compare":{"left":{"Tally":{"event":"PlusOneCountersPut","who":"You","filter":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"op":"Ge","right":1}},"mods":[{"ModifyPT":{"power":2,"toughness":0}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules),
    )
}

#[rstest]
#[case::stays("Wolf", None, 3)]
#[case::dies("Wolf", Some(r#"{"Destroy":{"what":{"Target":0}}}"#), 3)]
#[case::is_bounced("Wolf", Some(r#"{"Move":{"what":{"Target":0},"to":"Hand"}}"#), 3)]
#[case::an_opponents_creature_you_then_take(
    "Ogre",
    Some(r#"{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}}"#),
    1
)]
fn counters_count_as_the_creature_was_when_they_were_put(
    #[values(2, 3, 4)] seats: usize,
    #[case] grown: &str,
    #[case] then: Option<&str>,
    #[case] power: i32,
) -> Result<(), ScriptError> {
    let creature = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;
    let grow = spell(
        "Grow",
        CardType::Instant,
        creature,
        r#"[{"PutCounters":{"counter":"+1/+1","on":{"Target":0}}}]"#,
    );
    let after = spell(
        "After",
        CardType::Instant,
        creature,
        &format!("[{}]", then.unwrap_or(r#"{"GainLife":{"amount":1}}"#)),
    );
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [grow.clone(), after.clone()])
            .battlefield(0, [ready(watcher()), ready(free_creature("Wolf", 2, 2))])
            .battlefield(seats - 1, [ready(free_creature("Ogre", 5, 5))])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let target = driver.target_named(grown)?;
    driver.cast_at("Grow", &[target])?;
    settle(&mut driver)?;
    let target = driver.target_named(grown)?;
    driver.cast_at("After", &[target])?;
    settle(&mut driver)?;
    let watcher = on_battlefield(driver.state(), "Watcher").unwrap();
    assert_eq!(
        characteristics(driver.state(), watcher).unwrap().power,
        Some(power)
    );
    let setup = scenario().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
