//! Goal: bolster binds the creature it chose (`Effect::Bolster.as`), so "the chosen creature gains trample until end
//! of turn. When that creature deals combat damage to a player this turn, ..." reads that creature alone, whichever
//! creature attacks, at every table size (Optimus Prime, Autobot Leader).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A 5/5 with "Whenever you attack, bolster 2. The chosen creature gains trample until end of turn. When that creature
/// deals combat damage to a player this turn, you gain 5 life."
fn leader() -> Arc<CardDefinition> {
    let json = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[
        {"Bolster":{"count":2,"as":"chosen"}},
        {"Continuous":{"affected":{"Bound":"chosen"},"mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}},
        {"DelayedTrigger":{"when":{"DealsDamage":{"source":[{"Is":{"Bound":"chosen"}}],"to":"EachPlayer","combat":true}},"this_turn":true,"effects":[{"GainLife":{"amount":5}}]}}]}}"#;
    Arc::new(
        CardDefinition::new("Leader")
            .with_type(CardType::Creature)
            .with_power_toughness(5, 5)
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

#[rstest]
fn the_chosen_creature_alone_gets_trample_and_the_delayed_trigger(
    #[values("Leader", "Runt")] attacker: &str,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(leader()),
            ready(free_creature("Runt", 1, 1)),
            ready(free_creature("Ox", 3, 3)),
        ],
    );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 0, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(attacker, 1)])?;
    for _ in 0..400 {
        if driver.state().turn().step() >= Step::EndCombat {
            break;
        }
        driver.answer_passively()?;
    }
    let runt = driver.id_of("Runt")?;
    let counters = driver.state().object(runt).and_then(|o| {
        o.permanent()
            .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne))
    });
    assert_eq!(counters, Some(2), "the Runt has the least toughness");
    if driver.state().turn().step() == Step::EndCombat {
        assert!(mtg_engine::keywords::has_keyword(
            driver.state(),
            runt,
            &Keyword::Trample
        ));
        let ox = driver.id_of("Ox")?;
        assert!(!mtg_engine::keywords::has_keyword(
            driver.state(),
            ox,
            &Keyword::Trample
        ));
    }
    // Unblocked, the attacker deals combat damage to seat 1; only the Runt's damage gains the life.
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, if attacker == "Runt" { 25 } else { 20 });
    Ok(())
}
