//! Goal: rules of loyalty activations: each planeswalker activates one loyalty ability a turn, two under "twice each turn" (never more for two such rules, never under an opponent's), one more for each "once more" rule; "any time you could cast an instant" opens them on any turn to their controller only; an additional [+N] is one change of counters with the printed cost, so a [-N] needs N minus the addition; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A planeswalker with loyalty 3, "+1: You gain 1 life." and "-<minus>: You gain 5 life."
fn walker(minus: u32) -> Arc<CardDefinition> {
    let rules = [
        r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#.to_owned(),
        format!(
            r#"{{"Loyalty":{{"cost":{{"Minus":{minus}}},"effects":[{{"GainLife":{{"amount":5}}}}]}}}}"#
        ),
    ];
    let mut card = CardDefinition::new("Walker")
        .with_type(CardType::Planeswalker)
        .with_loyalty(3);
    for rule in rules {
        card = card.with_rules(serde_json::from_str::<Ability>(&rule).unwrap());
    }
    Arc::new(card)
}

/// An enchantment named `name` with the static rule `rule` (JSON of a `GameRule`).
fn rule_card(name: &str, rule: &str) -> Arc<CardDefinition> {
    let json = format!(r#"{{"Rule":{{"rule":{rule}}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_rules(serde_json::from_str::<Ability>(&json).unwrap()),
    )
}

/// Answers passively until nothing is on the stack and nothing waits.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn loyalty(driver: &Driver) -> u32 {
    let Ok(walker) = driver.id_of("Walker") else {
        return 0;
    };
    driver
        .state()
        .object(walker)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.counters_of(&CounterKind::Loyalty))
}

const TWICE: &str = r#"{"LoyaltyTwice":"You"}"#;
const ONCE_MORE: &str = r#"{"LoyaltyOnceMore":"You"}"#;

#[rstest]
#[case::once(&[], &[], 1)]
#[case::twice(&[TWICE], &[], 2)]
#[case::twice_twice(&[TWICE, TWICE], &[], 2)]
#[case::an_opponent_s_twice(&[], &[TWICE], 1)]
#[case::once_more(&[ONCE_MORE], &[], 2)]
#[case::once_more_twice(&[ONCE_MORE, ONCE_MORE], &[], 3)]
#[case::twice_and_once_more(&[TWICE, ONCE_MORE], &[], 3)]
fn a_planeswalker_activates_as_many_loyalty_abilities_as_the_rules_allow(
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] allowed: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let named = |rules: &[&str], seat: usize| -> Vec<_> {
        rules
            .iter()
            .enumerate()
            .map(|(i, rule)| ready(rule_card(&format!("Rule {seat} {i}"), rule)))
            .collect()
    };
    let mut board = vec![ready(walker(1))];
    board.extend(named(mine, 0));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, board)
            .battlefield(1, named(theirs, 1)),
    )?;
    let mut activated = 0;
    while driver.offered_abilities("Walker")?.contains(&0) {
        driver.begin_ability("Walker", 0)?;
        settle(&mut driver)?;
        activated += 1;
        assert!(activated <= 5, "no end to the activations");
    }
    assert_eq!(activated, allowed);
    assert_eq!(loyalty(&driver), 3 + u32::try_from(allowed).unwrap());
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life(),
        20 + i32::try_from(allowed).unwrap()
    );
    Ok(())
}

#[rstest]
#[case::plus_one(0, 1, 0, Some(4))]
#[case::plus_one_and_one_more(0, 1, 1, Some(5))]
#[case::minus_three(1, 3, 0, Some(0))]
#[case::minus_three_and_one_more(1, 3, 1, Some(1))]
#[case::minus_four_needs_four(1, 4, 0, None)]
#[case::minus_four_and_one_more(1, 4, 1, Some(0))]
#[case::minus_five_and_two_more(1, 5, 2, Some(0))]
fn an_additional_loyalty_cost_is_one_change_with_the_printed_cost(
    #[case] ability: usize,
    #[case] minus: u32,
    #[case] plus: u32,
    #[case] after: Option<u32>,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // The walker enters with 3 loyalty; `after` is what it holds once the cost is paid and the ability resolved.
    let mut board = vec![ready(walker(minus))];
    if plus > 0 {
        board.push(ready(rule_card(
            "Carth",
            &format!(r#"{{"LoyaltyCostMore":{{"who":"You","plus":{plus}}}}}"#),
        )));
    }
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, board))?;
    let offered = driver.offered_abilities("Walker")?.contains(&ability);
    assert_eq!(offered, after.is_some());
    if let Some(after) = after {
        driver.begin_ability("Walker", ability)?;
        settle(&mut driver)?;
        assert_eq!(loyalty(&driver), after);
    }
    Ok(())
}

/// Whether the deciding player is `seat` and is offered the walker's +1.
fn walker_offered_to(driver: &Driver, seat: usize) -> bool {
    let decision = driver.decision().unwrap();
    decision.player == pid(seat)
        && matches!(&decision.kind, DecisionKind::Priority(_))
        && driver.offered_abilities("Walker").unwrap().contains(&0)
}

#[rstest]
#[case::sorcery_speed_only(None, false)]
#[case::my_emblem_rule(Some(0), true)]
#[case::an_opponent_s_rule(Some(1), false)]
fn loyalty_abilities_at_instant_speed_only_under_their_controller_s_rule(
    #[case] owner: Option<usize>,
    #[case] can: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rule = || ready(rule_card("Emblem", r#"{"LoyaltyAsInstant":"You"}"#));
    let mut scenario = Scenario::new(seats).at(2, 1, Step::Upkeep);
    let mut mine = vec![ready(walker(1))];
    let mut theirs = Vec::new();
    match owner {
        Some(0) => mine.push(rule()),
        Some(_) => theirs.push(rule()),
        None => {}
    }
    scenario = scenario.battlefield(0, mine).battlefield(1, theirs);
    let mut driver = Driver::from_scenario(scenario)?;
    // The active player passes; seat 0 gets priority in the opponent's upkeep.
    for _ in 0..seats {
        if driver.decision()?.player == pid(0) {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(walker_offered_to(&driver, 0), can);
    if can {
        driver.begin_ability("Walker", 0)?;
        settle(&mut driver)?;
        assert_eq!(loyalty(&driver), 4);
    }
    Ok(())
}

#[rstest]
fn an_aura_copies_each_loyalty_ability_of_the_planeswalker_it_enchants(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let talent = mtg_testkit::corpus::card("Rowan's Talent").unwrap();
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        [
            ready(walker(1)),
            mtg_testkit::attached_to(ready(talent), 0, 0),
        ],
    ))?;
    driver.begin_ability("Walker", 0)?;
    settle(&mut driver)?;
    assert_eq!(loyalty(&driver), 4, "the copy pays no cost");
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 22);
    Ok(())
}
