//! Goal: an `If` runs the effects of its `then` when its condition holds and those of its `otherwise` when it does not, reading the state the earlier steps left, at tables of two, three and four seats, for conditions on life, the board, whose turn it is and a chosen target, for conditions inside conditions and for effects before and after it; and a condition the engine cannot read (loyalty, being kicked) puts its card outside what the capabilities cover, wherever the condition stands, so no game offers it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::validate::used_primitives;
use mtg_core::card::CardType;
use mtg_engine::caps;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// An instant for `{0}` whose one ability is a spell with these targets and effects, both as JSON.
fn spell(name: &str, targets: &str, effects: &str) -> Arc<CardDefinition> {
    common::spell(name, CardType::Instant, targets, effects)
}

/// `seats` seats; seat 0 holds `card` and `creatures` free 2/2s, and the turn is its own or seat 1's. Whoever holds
/// priority before seat 0 has passed.
fn table(seats: usize, card: Arc<CardDefinition>, creatures: usize, own_turn: bool) -> Scenario {
    let active = usize::from(!own_turn);
    let bears: Vec<_> = (0..creatures)
        .map(|n| ready(free_creature(&format!("Bear {n}"), 2, 2)))
        .collect();
    Scenario::new(seats)
        .at(1, active, Step::Main1)
        .hand(0, [card])
        .battlefield(0, bears)
}

fn until_seat_0_has_priority(driver: &mut Driver) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    Ok(())
}

fn resolve_all(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::life_just_enough(
    r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}"#,
    20,
    0,
    true,
    true
)]
#[case::life_one_short(
    r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}"#,
    19,
    0,
    true,
    false
)]
#[case::life_far_above(
    r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}"#,
    40,
    0,
    true,
    true
)]
#[case::the_other_way_round(
    r#"{"Not":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}}"#,
    19,
    0,
    true,
    true
)]
#[case::two_creatures_of_mine(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}"#, 20, 2, true, true)]
#[case::one_creature_of_mine(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}"#, 20, 1, true, false)]
#[case::on_my_turn_in_my_turn(r#""YourTurn""#, 20, 0, true, true)]
#[case::on_my_turn_in_another_turn(r#""YourTurn""#, 20, 0, false, false)]
#[case::not_my_turn_in_another_turn(r#"{"Not":"YourTurn"}"#, 20, 0, false, true)]
#[case::all_of_two(
    r#"{"All":["YourTurn",{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}]}"#,
    20,
    0,
    true,
    true
)]
#[case::all_of_two_one_short(
    r#"{"All":["YourTurn",{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}]}"#,
    19,
    0,
    true,
    false
)]
#[case::any_of_two(
    r#"{"Any":[{"Not":"YourTurn"},{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}]}"#,
    19,
    0,
    false,
    true
)]
#[case::any_of_two_neither(
    r#"{"Any":[{"Not":"YourTurn"},{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}]}"#,
    19,
    0,
    true,
    false
)]
fn the_condition_picks_what_happens(
    #[case] condition: &str,
    #[case] life: i32,
    #[case] creatures: usize,
    #[case] own_turn: bool,
    #[case] then: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // 5 for the `then`, -5 for the `otherwise`, and 1 for the effect after the If, which happens either way.
    let fork = spell(
        "Fork",
        "[]",
        &format!(
            r#"[{{"If":{{"condition":{condition},"then":[{{"GainLife":{{"amount":5}}}}],"otherwise":[{{"LoseLife":{{"amount":5}}}}]}}}},{{"GainLife":{{"amount":1}}}}]"#
        ),
    );
    let mut driver = Driver::from_scenario(table(seats, fork, creatures, own_turn).life(0, life))?;
    until_seat_0_has_priority(&mut driver)?;
    driver.cast("Fork")?;
    resolve_all(&mut driver)?;
    driver.expect_life(0, life + if then { 5 } else { -5 } + 1)?;
    for seat in 1..seats {
        driver.expect_life(seat, 20)?;
    }
    driver.expect_zone(Zone::Graveyard(pid(0)), &["Fork"])
}

#[rstest]
#[case::list_of_nothing_taken("[]", r#"[{"LoseLife":{"amount":5}}]"#, 20, 1)]
#[case::list_of_nothing_not_taken("[]", r#"[{"LoseLife":{"amount":5}}]"#, 19, -4)]
#[case::no_otherwise_taken(r#"[{"GainLife":{"amount":5}}]"#, "[]", 20, 6)]
#[case::no_otherwise_not_taken(r#"[{"GainLife":{"amount":5}}]"#, "[]", 19, 1)]
#[case::two_effects_in_a_list(
    r#"[{"GainLife":{"amount":5}},{"GainLife":{"amount":6}}]"#,
    "[]",
    20,
    12
)]
fn an_empty_list_goes_on_to_the_next_effect(
    #[case] then: &str,
    #[case] otherwise: &str,
    #[case] life: i32,
    #[case] change: i32,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let fork = spell(
        "Fork",
        "[]",
        &format!(
            r#"[{{"If":{{"condition":{{"Compare":{{"left":{{"Life":"You"}},"op":"Ge","right":20}}}},"then":{then},"otherwise":{otherwise}}}}},{{"GainLife":{{"amount":1}}}}]"#
        ),
    );
    let mut driver = Driver::from_scenario(table(seats, fork, 0, true).life(0, life))?;
    driver.cast("Fork")?;
    resolve_all(&mut driver)?;
    driver.expect_life(0, life + change)
}

#[rstest]
// The 5 gained first can lift the life total past the line the inner condition draws, and only the state as the
// earlier steps left it decides.
#[case::own_turn_reaching_it(true, 20, 36)]
#[case::own_turn_one_short(true, 19, 15)]
#[case::another_turn_reaching_it(false, 20, 46)]
#[case::another_turn_one_short(false, 19, 45)]
fn a_condition_reads_what_the_steps_before_it_did_and_conditions_nest(
    #[case] own_turn: bool,
    #[case] life: i32,
    #[case] after: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let maze = spell(
        "Maze",
        "[]",
        r#"[
            {"GainLife":{"amount":5}},
            {"If":{"condition":"YourTurn",
                "then":[{"If":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":25}},
                    "then":[{"GainLife":{"amount":10}}],"otherwise":[{"LoseLife":{"amount":10}}]}}],
                "otherwise":[{"GainLife":{"amount":20}}]}},
            {"GainLife":{"amount":1}}
        ]"#,
    );
    let mut driver = Driver::from_scenario(table(seats, maze, 0, own_turn).life(0, life))?;
    until_seat_0_has_priority(&mut driver)?;
    driver.cast("Maze")?;
    resolve_all(&mut driver)?;
    driver.expect_life(0, after)
}

#[rstest]
#[case::a_big_creature("Ogre", true)]
#[case::a_small_one("Bear", false)]
fn a_condition_can_ask_about_the_target_of_the_spell(
    #[case] target: &str,
    #[case] destroyed: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let smite = spell(
        "Smite",
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        r#"[{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Compare":{"stat":"Power","op":"Ge","to":3}}]}},
            "then":[{"Destroy":{"what":{"Target":0}}}],"otherwise":[{"GainLife":{"amount":2}}]}}]"#,
    );
    let scenario = table(seats, smite, 0, true).battlefield(
        1,
        [
            ready(free_creature("Ogre", 3, 3)),
            ready(free_creature("Bear", 2, 2)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let chosen = driver.target_named(target)?;
    driver.cast_at("Smite", &[chosen])?;
    resolve_all(&mut driver)?;
    if destroyed {
        driver.expect_zone(Zone::Graveyard(pid(1)), &["Ogre"])?;
        driver.expect_life(0, 20)
    } else {
        driver.expect_zone(Zone::Graveyard(pid(1)), &[])?;
        driver.expect_life(0, 22)
    }
}

/// An `If` with this condition, as a spell.
fn with_condition(condition: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"If":{{"condition":{condition},"then":[{{"Draw":{{}}}}]}}}}]}}}}"#
    )
}

const COUNTERS: &str =
    r#"{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":1}}"#;
const LOYALTY: &str =
    r#"{"Compare":{"left":{"Stat":{"of":"This","stat":"Loyalty"}},"op":"Ge","right":1}}"#;
const LOYALTY_OF_A_MATCH: &str = r#"{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"Loyalty","op":"Ge","to":1}}]}}"#;

#[rstest]
#[case::your_turn(with_condition(r#""YourTurn""#), &[])]
#[case::life(with_condition(r#"{"Compare":{"left":{"Life":"You"},"op":"Ge","right":20}}"#), &[])]
#[case::a_match_on_power(with_condition(r#"{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"Power","op":"Ge","to":3}}]}}"#), &[])]
#[case::counters(with_condition(COUNTERS), &[])]
#[case::loyalty(with_condition(LOYALTY), &["Stat.Loyalty"])]
#[case::loyalty_of_a_match(with_condition(LOYALTY_OF_A_MATCH), &["Stat.Loyalty"])]
#[case::kicked(with_condition(r#""Kicked""#), &["Condition.Kicked"])]
#[case::not_kicked(with_condition(r#"{"Not":"Kicked"}"#), &["Condition.Kicked"])]
#[case::one_of_all(with_condition(&format!(r#"{{"All":["YourTurn",{COUNTERS}]}}"#)), &[])]
#[case::one_of_any(with_condition(&format!(r#"{{"Any":[{LOYALTY},"Kicked"]}}"#)), &["Condition.Kicked", "Stat.Loyalty"])]
#[case::inside_the_then_of_another_if(
    format!(r#"{{"Spell":{{"effects":[{{"If":{{"condition":"YourTurn","then":[{{"If":{{"condition":{COUNTERS},"then":[]}}}}]}}}}]}}}}"#),
    &[]
)]
#[case::inside_the_otherwise_of_another_if(
    format!(r#"{{"Spell":{{"effects":[{{"If":{{"condition":"YourTurn","then":[],"otherwise":[{{"If":{{"condition":{LOYALTY},"then":[]}}}}]}}}}]}}}}"#),
    &["Stat.Loyalty"]
)]
#[case::inside_a_may(
    format!(r#"{{"Spell":{{"effects":[{{"May":{{"effects":[{{"If":{{"condition":{COUNTERS},"then":[]}}}}]}}}}]}}}}"#),
    &[]
)]
#[case::the_intervening_if_of_a_trigger(
    format!(r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"if":{COUNTERS},"effects":[{{"Draw":{{}}}}]}}}}"#),
    &[]
)]
fn a_card_that_needs_a_condition_the_engine_cannot_read_is_not_covered(
    #[case] ability: String,
    #[case] lacks: &[&str],
) {
    let ability: Ability = serde_json::from_str(&ability).unwrap();
    let mut missing = caps().missing(&used_primitives(&ability));
    missing.sort();
    assert_eq!(missing, lacks);
}
