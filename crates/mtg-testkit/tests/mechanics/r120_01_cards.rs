//! Goal: at 2 to 4 seats, a trigger on counters of one kind put on a permanent "for the first time each turn" counts
//! only the puts of that kind (U.S.S. Enterprise-D), whatever other counters came before or in the same resolution, and
//! a trigger on your random actions (Chira) fires once for each roll, flip, random discard, random modes and random
//! targets of yours, never for a chosen discard or an opponent's roll, and La Forge counts the permanents you control
//! that your spell put a counter on this turn, and Yume gives only the first spell of your turn assist and a card to you
//! and the helper when another player paid, and Matoc exiles once for each source that dealt one recipient exactly 2;
//! each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Answers until the stack and the pending triggers are empty: `pick` answers the decisions it knows, the rest are
/// answered passively.
fn r120_resolve(
    driver: &mut Driver,
    pick: impl Fn(&Decision) -> Option<Answer>,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let busy =
            !driver.state().stack().is_empty() || !driver.state().pending_triggers().is_empty();
        let decision = driver.decision()?.clone();
        if !matches!(decision.kind, DecisionKind::Priority(_))
            && let Some(answer) = pick(&decision)
        {
            driver.answer(answer)?;
            continue;
        }
        match &decision.kind {
            DecisionKind::Priority(_) if busy => {
                driver.pass()?;
            }
            DecisionKind::Priority(_) => return Ok(()),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the resolution".into()))
}

/// The log replays to the live state.
fn r120_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// A free card of the type with the one ability.
fn r120_card(name: &str, kind: CardType, rules: &str) -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    )
}

/// The table: each seat's library of forests.
fn r120_table(seats: usize, forests: usize) -> Scenario {
    (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..forests).map(|_| forest()))
    })
}

/// A sorcery that puts one counter of each kind, in order, on each artifact.
fn r120_putting(name: &str, kinds: &[&str]) -> Arc<CardDefinition> {
    let effects: Vec<String> = kinds
        .iter()
        .map(|kind| {
            format!(
                r#"{{"PutCounters":{{"on":{{"Each":[{{"Type":"Artifact"}}]}},"counter":"{kind}"}}}}"#
            )
        })
        .collect();
    r120_card(
        name,
        CardType::Sorcery,
        &format!(r#"{{"Spell":{{"effects":[{}]}}}}"#, effects.join(",")),
    )
}

#[rstest]
#[case::charge_then_charge(&["charge"], &["charge"], 1)]
#[case::plus_then_charge(&["+1/+1"], &["charge"], 1)]
#[case::plus_and_charge_together(&["+1/+1", "charge"], &["charge"], 1)]
#[case::charge_twice_together(&["charge", "charge"], &[], 1)]
#[case::plus_only(&["+1/+1"], &["+1/+1", "+1/+1"], 0)]
#[case::none(&[], &[], 0)]
fn r120_the_first_charge_counters_of_a_turn_trigger_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] first: &[&str],
    #[case] second: &[&str],
    #[case] triggers: i32,
) -> Result<(), ScriptError> {
    let ship = r120_card(
        "Ship",
        CardType::Artifact,
        r#"{"Triggered":{"when":{"Nth":{"event":{"CountersPut":{"what":["This"],"counter":"charge"}},"nth":1}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    );
    let scenario = r120_table(seats, 10)
        .hand(
            0,
            [r120_putting("First", first), r120_putting("Second", second)],
        )
        .battlefield(0, [ready(ship)])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    for name in ["First", "Second"] {
        driver.cast(name)?;
        r120_resolve(&mut driver, |_| None)?;
    }
    r120_replays(&driver);
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life() - life,
        triggers
    );
    Ok(())
}

#[rstest]
#[case::two_dice_in_one_roll(r#"[{"RollDice":{"sides":6,"dice":2}}]"#, 1)]
#[case::two_rolls(r#"[{"RollDice":{"sides":6}},{"RollDice":{"sides":20}}]"#, 2)]
#[case::a_coin(r#"[{"FlipCoin":{}}]"#, 1)]
#[case::a_coin_and_a_die(r#"[{"FlipCoin":{}},{"RollDice":{"sides":6}}]"#, 2)]
#[case::two_cards_at_random(r#"[{"Discard":{"count":2,"random":true}}]"#, 1)]
#[case::a_chosen_discard(r#"[{"Discard":{"count":2}}]"#, 0)]
#[case::an_opponents_roll(r#"[{"RollDice":{"who":"Opponents","sides":6}}]"#, 0)]
#[case::modes_at_random(
    r#"[{"Modal":{"at_random":true,"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"Mill":{}}]}]}}]"#,
    1
)]
#[case::nothing_random(r#"[{"Draw":{}}]"#, 0)]
fn r120_each_random_instruction_of_yours_triggers_once(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &str,
    #[case] triggers: i32,
) -> Result<(), ScriptError> {
    let watcher = r120_card(
        "Watcher",
        CardType::Artifact,
        r#"{"Triggered":{"when":{"R120RandomAction":{"who":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    );
    let test = r120_card(
        "Test",
        CardType::Sorcery,
        &format!(r#"{{"Spell":{{"effects":{effects}}}}}"#),
    );
    let scenario = r120_table(seats, 10)
        .hand(0, [test, forest(), forest(), forest()])
        .battlefield(0, [ready(watcher)])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Test")?;
    r120_resolve(&mut driver, |_| None)?;
    r120_replays(&driver);
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life() - life,
        triggers
    );
    Ok(())
}

#[rstest]
#[case::none(0, 0)]
#[case::one_of_two(1, 2)]
#[case::both(2, 2)]
fn r120_la_forge_counts_the_permanents_you_put_a_counter_on(
    #[values(2, 3, 4)] seats: usize,
    #[case] countered: usize,
    #[case] artifacts: usize,
) -> Result<(), ScriptError> {
    // The sorcery puts a counter on each of the first `countered` artifacts, and on every opponent's artifact.
    let test = r120_card(
        "Test",
        CardType::Sorcery,
        r#"{"Spell":{"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Artifact"},{"Subtype":"Gear"}]},"counter":"charge"}}]}}"#,
    );
    let gear = |name: &str| {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Artifact)
                .with_subtype("Gear")
                .with_mana_cost(cost(0, &[])),
        )
    };
    let mine = (0..artifacts).map(|at| {
        if at < countered {
            ready(gear(&format!("Gear {at}")))
        } else {
            ready(r120_card(
                &format!("Plain {at}"),
                CardType::Artifact,
                r#"{"Keyword":"Flying"}"#,
            ))
        }
    });
    let scenario = r120_table(seats, 10)
        .hand(0, [test])
        .battlefield(0, mine)
        .battlefield(0, [ready(card("La Forge, Thoughtful as Ever").unwrap())])
        .battlefield(1, [ready(gear("Their Gear"))])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Test")?;
    r120_resolve(&mut driver, |_| None)?;
    driver.begin_activation("La Forge, Thoughtful as Ever")?;
    r120_resolve(&mut driver, |_| None)?;
    r120_replays(&driver);
    let faeries = driver
        .state()
        .objects()
        .filter(|object| object.card().name == "Faerie")
        .count();
    assert_eq!(faeries, countered);
    Ok(())
}

#[rstest]
fn r120_a_target_selected_at_random_triggers_once(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] bears: usize,
) -> Result<(), ScriptError> {
    let watcher = r120_card(
        "Watcher",
        CardType::Artifact,
        r#"{"Triggered":{"when":{"R120RandomAction":{"who":"You"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    );
    let test = r120_card(
        "Test",
        CardType::Sorcery,
        r#"{"Spell":{"targets":[{"what":{"AtRandom":{"Object":[{"Type":"Creature"}]}}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#,
    );
    let creatures = (0..bears).map(|at| {
        ready(mtg_testkit::cards::free_creature(
            &format!("Bear {at}"),
            2,
            2,
        ))
    });
    let scenario = r120_table(seats, 10)
        .hand(0, [test])
        .battlefield(0, [ready(watcher)])
        .battlefield(1, creatures)
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Test")?;
    r120_resolve(&mut driver, |_| None)?;
    r120_replays(&driver);
    assert_eq!(driver.state().player(pid(0)).unwrap().life() - life, 1);
    Ok(())
}

/// Casts `name`, answering the assist questions with `helper` (0 is no one, 1 the first helper) and `amount`; whether
/// assist was offered.
fn r120_cast_assisted(
    driver: &mut Driver,
    name: &str,
    helper: usize,
    amount: u32,
) -> Result<bool, ScriptError> {
    driver.begin_cast(name)?;
    let mut offered = false;
    for _ in 0..10 {
        let decision = driver.decision()?;
        match &decision.kind {
            DecisionKind::ChooseOption(_) => {
                offered = true;
                driver.answer(Answer::ChooseOption(mtg_engine::choose::OptionChoice(
                    helper,
                )))?;
            }
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(
                    mtg_engine::choose::NumberChoice::Number(amount),
                ))?;
            }
            DecisionKind::PayMana(_) if decision.player != pid(0) => {
                driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
            }
            DecisionKind::PayMana(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    r120_resolve(driver, |_| None)?;
    Ok(offered)
}

#[rstest]
#[case::helped(1, 2, 1)]
#[case::no_one(0, 0, 0)]
fn r120_yume_gives_the_first_spell_assist_and_a_card_to_each_when_helped(
    #[values(2, 3, 4)] seats: usize,
    #[case] helper: usize,
    #[case] amount: u32,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let giant = |name: &str| {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_mana_cost(mtg_core::mana::ManaCost::parse("{2}{B}").unwrap())
                .with_power_toughness(3, 3),
        )
    };
    let lands = |count: usize, land: fn() -> Arc<CardDefinition>| {
        (0..count).map(move |_| ready(land())).collect::<Vec<_>>()
    };
    let scenario = r120_table(seats, 10)
        .hand(0, [giant("First Giant"), giant("Second Giant")])
        .battlefield(0, [ready(card("Yume, Chronicler of Valor").unwrap())])
        .battlefield(0, lands(6, mtg_testkit::cards::swamp))
        .battlefield(1, lands(2, forest))
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = |driver: &Driver, seat: usize| {
        driver
            .state()
            .zone(mtg_engine::zones::Zone::Hand(pid(seat)))
            .len()
    };
    assert!(r120_cast_assisted(
        &mut driver,
        "First Giant",
        helper,
        amount
    )?);
    assert_eq!(hand(&driver, 0), 1 + drawn);
    assert_eq!(hand(&driver, 1), drawn);
    // The second spell of the turn has no assist.
    assert!(!r120_cast_assisted(&mut driver, "Second Giant", 0, 0)?);
    r120_replays(&driver);
    Ok(())
}

#[rstest]
#[case::two_to_one_opponent(r#"[{"DealDamage":{"to":{"Target":0},"amount":2}}]"#, true, 1)]
#[case::three_to_one_opponent(r#"[{"DealDamage":{"to":{"Target":0},"amount":3}}]"#, true, 0)]
#[case::two_to_each_opponent(r#"[{"DealDamage":{"to":"Opponents","amount":2}}]"#, false, 1)]
#[case::two_separate_hits(
    r#"[{"DealDamage":{"to":{"Target":0},"amount":2}},{"DealDamage":{"to":{"Target":0},"amount":2}}]"#,
    true,
    2
)]
fn r120_matoc_exiles_once_for_each_source_that_dealt_exactly_two(
    #[values(2, 3, 4)] seats: usize,
    #[case] effects: &str,
    #[case] targeted: bool,
    #[case] exiled: usize,
) -> Result<(), ScriptError> {
    let targets = if targeted {
        r#","targets":[{"what":"Player"}]"#
    } else {
        ""
    };
    let test = r120_card(
        "Test",
        CardType::Sorcery,
        &format!(r#"{{"Spell":{{"effects":{effects}{targets}}}}}"#),
    );
    let scenario = r120_table(seats, 10)
        .hand(0, [test])
        .battlefield(0, [ready(card("Matoc, Lavamancer").unwrap())])
        .at(2, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    if targeted {
        let foe = driver.target_player(1);
        driver.cast_at("Test", &[foe])?;
    } else {
        driver.cast("Test")?;
    }
    r120_resolve(&mut driver, |_| None)?;
    r120_replays(&driver);
    let exile = driver.state().zone(mtg_engine::zones::Zone::Exile).len();
    assert_eq!(exile, exiled);
    Ok(())
}
