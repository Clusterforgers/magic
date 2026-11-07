//! Goal: the facts of round 10 pack g10c play whole at tables of two, three and four seats: a spell gains the damage it dealt this way and no more, a player who targets an opponent or what an opponent controls has committed a crime this turn and one who targets only their own has not, +1/+1 counters put this turn count apart from other counters, the damage a target player took this turn is theirs alone, and each log replays to the state it was played to.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::DecisionKind;
use mtg_engine::replay;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{on_battlefield, pid, spell};

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

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn power(driver: &Driver, name: &str) -> Option<i32> {
    let id = on_battlefield(driver.state(), name).unwrap();
    characteristics(driver.state(), id).unwrap().power
}

fn replays(driver: &Driver, scenario: Scenario) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// A 1/1 that gets +2/+0 while the condition holds.
fn watcher(condition: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Static":{{"affected":"This","condition":{condition},"mods":[{{"ModifyPT":{{"power":2,"toughness":0}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules),
    )
}

#[rstest]
fn a_spell_gains_the_damage_it_dealt_this_way(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let syphon = spell(
        "Syphon",
        CardType::Sorcery,
        "[]",
        r#"[{"DealDamage":{"amount":2,"to":"Opponents"}},{"GainLife":{"amount":"DamageDealtThisWay"}}]"#,
    );
    // Gaining "the damage dealt this way" with no damage dealt gains nothing.
    let idle = spell(
        "Idle",
        CardType::Sorcery,
        "[]",
        r#"[{"GainLife":{"amount":"DamageDealtThisWay"}}]"#,
    );
    let scenario = || Scenario::new(seats).hand(0, [syphon.clone(), idle.clone()]);
    let mut driver = Driver::from_scenario(scenario())?;
    let (mine, theirs) = (life(&driver, 0), life(&driver, 1));
    driver.cast("Syphon")?;
    settle(&mut driver)?;
    let opponents = i32::try_from(seats - 1).unwrap();
    assert_eq!(life(&driver, 0), mine + 2 * opponents);
    assert_eq!(life(&driver, 1), theirs - 2);
    driver.cast("Idle")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), mine + 2 * opponents);
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::an_opponents_creature("Ogre", Some(3))]
#[case::your_own_creature("Wolf", Some(1))]
fn targeting_an_opponents_creature_is_a_crime_this_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] victim: &str,
    #[case] watched: Option<i32>,
) -> Result<(), ScriptError> {
    let crime =
        r#"{"Compare":{"left":{"Tallied":{"what":"Crimes","who":"You"}},"op":"Ge","right":1}}"#;
    let poke = spell(
        "Poke",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"DealDamage":{"amount":1,"to":{"Target":0}}}]"#,
    );
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [poke.clone()])
            .battlefield(
                0,
                [ready(watcher(crime)), ready(free_creature("Wolf", 5, 5))],
            )
            .battlefield(seats - 1, [ready(free_creature("Ogre", 5, 5))])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    assert_eq!(power(&driver, "Watcher"), Some(1));
    let target = driver.target_named(victim)?;
    driver.cast_at("Poke", &[target])?;
    // The crime is committed as the spell is cast, before it resolves.
    assert_eq!(power(&driver, "Watcher"), watched);
    settle(&mut driver)?;
    assert_eq!(power(&driver, "Watcher"), watched);
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::plus_one("+1/+1", Some(3))]
#[case::another_kind("charge", Some(1))]
fn plus_one_counters_put_this_turn_count_apart(
    #[values(2, 3, 4)] seats: usize,
    #[case] counter: &str,
    #[case] watched: Option<i32>,
) -> Result<(), ScriptError> {
    let put = r#"{"Compare":{"left":{"Tally":{"event":"PlusOneCountersPut","who":"You","filter":[{"Type":"Creature"}]}},"op":"Ge","right":1}}"#;
    let grow = spell(
        "Grow",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        &format!(r#"[{{"PutCounters":{{"counter":"{counter}","on":{{"Target":0}}}}}}]"#),
    );
    let scenario = || {
        Scenario::new(seats)
            .hand(0, [grow.clone()])
            .battlefield(0, [ready(watcher(put))])
            .battlefield(1, [ready(free_creature("Ogre", 5, 5))])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let target = driver.target_named("Ogre")?;
    driver.cast_at("Grow", &[target])?;
    settle(&mut driver)?;
    assert_eq!(power(&driver, "Watcher"), watched);
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
fn a_target_player_loses_the_damage_they_took_this_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let burn = spell(
        "Burn",
        CardType::Instant,
        r#"[{"what":"Player"}]"#,
        r#"[{"DealDamage":{"amount":3,"to":{"Target":0}}}]"#,
    );
    let punish = spell(
        "Punish",
        CardType::Instant,
        r#"[{"what":"Player"}]"#,
        r#"[{"LoseLife":{"amount":{"Tallied":{"what":"DamageTaken","who":{"Target":0}}},"who":{"Target":0}}}]"#,
    );
    let last = seats - 1;
    let scenario = || Scenario::new(seats).hand(0, [burn.clone(), punish.clone()]);
    let mut driver = Driver::from_scenario(scenario())?;
    let start = life(&driver, last);
    driver.cast_at("Burn", &[mtg_engine::choose::Target::Player(pid(last))])?;
    settle(&mut driver)?;
    driver.cast_at("Punish", &[mtg_engine::choose::Target::Player(pid(last))])?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, last), start - 6);
    // A player who took no damage loses nothing.
    if seats > 2 {
        assert_eq!(life(&driver, 1), start);
    }
    replays(&driver, scenario());
    Ok(())
}

#[rstest]
#[case::four_in_one_spell(&[4], 3)]
#[case::four_in_two_spells(&[2, 2], 3)]
#[case::three_kill_it_too(&[3], 0)]
fn a_dead_creature_counts_the_damage_dealt_to_it_this_turn(
    #[values(2, 3, 4)] seats: usize,
    #[case] hits: &[u32],
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let trigger: Ability = serde_json::from_str(
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"if":{"Compare":{"left":{"Tallied":{"what":"DamageDealtTo","who":"This"}},"op":"Ge","right":4}},"effects":[{"GainLife":{"amount":3}}]}}"#,
    )
    .unwrap();
    let zubera = Arc::new(
        CardDefinition::new("Zubera")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 3)
            .with_rules(trigger),
    );
    let bolts: Vec<_> = hits
        .iter()
        .enumerate()
        .map(|(n, hit)| {
            spell(
                &format!("Bolt {n}"),
                CardType::Instant,
                r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
                &format!(r#"[{{"DealDamage":{{"amount":{hit},"to":{{"Target":0}}}}}}]"#),
            )
        })
        .collect();
    let scenario = || {
        Scenario::new(seats)
            .hand(0, bolts.clone())
            .battlefield(0, [ready(zubera.clone())])
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let start = life(&driver, 0);
    for n in 0..hits.len() {
        let target = driver.target_named("Zubera")?;
        driver.cast_at(&format!("Bolt {n}"), &[target])?;
        settle(&mut driver)?;
    }
    assert_eq!(on_battlefield(driver.state(), "Zubera"), None);
    assert_eq!(life(&driver, 0), start + gained);
    replays(&driver, scenario());
    Ok(())
}
