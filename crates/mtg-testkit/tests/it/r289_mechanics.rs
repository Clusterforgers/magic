//! Goal: pack r28-9's mechanics play whole at tables of two, three and four seats: a die of a named result triggers once for each die that shows it with the result as its amount and never for another result; "when the third plan counter is put on" triggers once, as the count passes three; "when this creature enters or the creature it haunts dies" triggers on the battlefield and again from exile when the haunted creature dies; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameSetup, replay};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn with(card: &Arc<CardDefinition>, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = (**card).clone();
    for rule in rules {
        let ability: Ability = serde_json::from_str(rule).unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

/// Passes and answers the first legal target until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// As `settle`, answering the target choices with `targets` in turn.
fn settle_choosing(
    driver: &mut Driver,
    targets: &[mtg_core::ids::ObjectId],
) -> Result<(), ScriptError> {
    let mut next = targets.iter();
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::ChooseTargets(options) => {
                let at = match next.next() {
                    Some(id) => mtg_engine::choose::Target::Object(
                        driver.state().object(*id).unwrap().reference(),
                    ),
                    None => options.legal[0],
                };
                driver.choose_targets(&[at])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn assert_replays(driver: &Driver, setup: &GameSetup) {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
}

/// "{0}: Roll a `sides`-sided die." and "Whenever you roll a `results`, you gain that much life."
fn roller(sides: u32, results: &str) -> Arc<CardDefinition> {
    with(
        &Arc::new(CardDefinition::new("Roller").with_type(CardType::Artifact)),
        &[
            &format!(
                r#"{{"Activated":{{"cost":"{{0}}","effects":[{{"RollDice":{{"sides":{sides},"results":[]}}}}]}}}}"#
            ),
            &format!(
                r#"{{"Triggered":{{"when":{{"DiceRolled":{{"who":"You","results":{results}}}}},"effects":[{{"GainLife":{{"amount":"Triggering"}}}}]}}}}"#
            ),
        ],
    )
}

#[rstest]
#[case::a_one_on_a_one_sided_die(1, "[1]", 21)]
#[case::one_or_two(1, "[1,2]", 21)]
#[case::not_a_two(1, "[2]", 20)]
fn a_named_result_triggers_with_that_much(
    #[values(2, 3, 4)] seats: usize,
    #[case] sides: u32,
    #[case] results: &str,
    #[case] expected_life: i32,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(roller(sides, results))])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.begin_ability("Roller", 0)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), expected_life);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

/// "{0}: Put a plan counter on this enchantment." and "When the third plan counter is put on this enchantment, you gain
/// 5 life."
fn scheme() -> Arc<CardDefinition> {
    with(
        &Arc::new(CardDefinition::new("Scheme").with_type(CardType::Enchantment)),
        &[
            r#"{"Activated":{"cost":"{0}","effects":[{"PutCounters":{"on":"This","counter":{"Other":"plan"}}}]}}"#,
            r#"{"Triggered":{"when":{"CountersPut":{"what":["This"],"counter":{"Other":"plan"},"reaching":3}},"effects":[{"GainLife":{"amount":5}}]}}"#,
        ],
    )
}

#[rstest]
#[case::two(2, 20)]
#[case::three(3, 25)]
#[case::five(5, 25)]
fn the_third_counter_triggers_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] puts: usize,
    #[case] expected_life: i32,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(scheme())])
    };
    let mut driver = Driver::from_scenario(table())?;
    for _ in 0..puts {
        driver.begin_ability("Scheme", 0)?;
        settle(&mut driver)?;
    }
    assert_eq!(life(&driver, 0), expected_life);
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}

/// Haunt and "When this creature enters or the creature it haunts dies, you gain 2 life."
fn haunter() -> Arc<CardDefinition> {
    with(
        &free_creature("Haunter", 1, 1),
        &[
            r#"{"Keyword":"Haunt"}"#,
            r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"HauntedBy":"This"}],"from":"Battlefield","to":"Graveyard"}}]},"effects":[{"GainLife":{"amount":2}}]}}"#,
        ],
    )
}

/// "{0}: Destroy target creature."
fn slayer() -> Arc<CardDefinition> {
    with(
        &Arc::new(CardDefinition::new("Slayer").with_type(CardType::Artifact)),
        &[
            r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
        ],
    )
}

#[rstest]
fn the_haunted_creature_dying_triggers_from_exile(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(slayer()), ready(free_creature("Victim", 2, 2))])
            .hand(0, [haunter()])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Haunter")?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, 0), 22, "it entered");
    let haunter = driver.id_of("Haunter")?;
    let victim = driver.id_of("Victim")?;
    driver.begin_ability("Slayer", 0)?;
    settle_choosing(&mut driver, &[haunter, victim])?;
    let exiled = driver.state().object(haunter).map(|o| o.zone());
    assert_eq!(
        life(&driver, 0),
        22,
        "dying haunts and gains nothing: {exiled:?}"
    );
    driver.begin_ability("Slayer", 0)?;
    settle_choosing(&mut driver, &[victim])?;
    assert_eq!(life(&driver, 0), 24, "the haunted creature died");
    assert_replays(&driver, &table().setup().unwrap());
    Ok(())
}
