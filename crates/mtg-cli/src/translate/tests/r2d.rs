//! Goal: the words that say who may activate an ability, how often and when (limits of two and three, any player, opponents, first combat, opponent's turn, a named condition, a cast restriction by count) read into the ability, and a value the table does not know stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn activated(cost: &str, extra: &str) -> Value {
    let text = format!(
        "Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ {cost} | NumCards$ 1 | {extra}\n"
    );
    abilities(&text).into_iter().next().unwrap_or_default()["Activated"].clone()
}

#[rstest]
#[case::twice("ActivationLimit$ 2", json!({"TimesPerTurn": 2}))]
#[case::three_times("ActivationLimit$ 3", json!({"TimesPerTurn": 3}))]
#[case::once("ActivationLimit$ 1", json!("OncePerTurn"))]
fn a_limit_of_several_uses_is_a_limit_per_turn(#[case] keys: &str, #[case] limit: Value) {
    assert_eq!(activated("1", keys)["limit"], limit);
}

#[test]
fn a_limit_of_four_stays_a_gap() {
    let text = "Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ 1 | ActivationLimit$ 4\n";
    assert_eq!(gaps(text), ["key:AB.ActivationLimit=4"]);
}

#[rstest]
#[case::any_player("1", "Activator$ Player", Some("Anyone"))]
#[case::opponents("1", "Activator$ Opponent", Some("Opponents"))]
#[case::opponents_by_property("1", "Activator$ Player.Opponent", Some("Opponents"))]
#[case::you("1", "Activator$ You", None)]
#[case::discarding("Discard<1/Card>", "Activator$ Player", Some("Anyone"))]
fn the_activators_are_the_players_the_word_names(
    #[case] cost: &str,
    #[case] keys: &str,
    #[case] by: Option<&str>,
) {
    assert_eq!(activated(cost, keys)["by"].as_str(), by);
}

#[rstest]
#[case::tap("T", "Activator$ Player", "key:AB.Activator-with-a-cost-of-the-source")]
#[case::sacrifice("1 Sac<1/CARDNAME>", "Activator$ Opponent", "key:AB.Activator-with-a-cost-of-the-source")]
#[case::remembered("1", "Activator$ Player.IsRemembered", "key:AB.Activator=Player.IsRemembered")]
fn a_player_cannot_pay_a_cost_of_a_permanent_they_do_not_control(
    #[case] cost: &str,
    #[case] keys: &str,
    #[case] gap: &str,
) {
    let text = format!("Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ {cost} | {keys}\n");
    assert_eq!(gaps(&text), [gap]);
}

#[rstest]
#[case::first_combat(
    "ActivationPhases$ Upkeep->BeginCombat | ActivationFirstCombat$ True",
    json!({"Any": [{"Step": "Upkeep"}, {"Step": "Draw"}, {"Step": "Main1"}, {"Step": "BeginCombat"}]})
)]
#[case::opponents_turn("OpponentTurn$ True", json!({"Not": "YourTurn"}))]
fn the_activation_words_of_a_turn_are_a_condition(#[case] keys: &str, #[case] expected: Value) {
    assert_eq!(activated("1", keys)["condition"], expected);
}

#[test]
fn first_combat_without_phases_stays_a_gap() {
    let text = "Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ 1 | ActivationFirstCombat$ True\n";
    assert_eq!(gaps(text), ["key:ActivationFirstCombat=True"]);
}

#[test]
fn a_named_activation_condition_is_the_condition() {
    let found = activated("1", "Activation$ Metalcraft");
    assert_eq!(found["condition"]["Compare"]["right"], json!(3));
}

#[test]
fn a_spell_cast_only_if_a_count_holds_is_a_cast_restriction() {
    let text = "Name:T\nTypes:Instant\nA:SP$ Draw | CheckSVar$ X | SVarCompare$ GE2\nSVar:X:Count$YourLifeTotal\n";
    let found = abilities(text);
    assert_eq!(found[0]["CastOnly"]["condition"]["Compare"]["op"], "Ge");
}
