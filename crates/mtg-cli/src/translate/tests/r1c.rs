//! Goal: the choice and loop words of pack C read as the engine runs them: an unless-cost any player may pay, a player chosen from a group, a choice among several lists, a mode restriction, and a may on an object verb; a word beyond them stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:A:DB$ GainLife | LifeAmount$ 2\nSVar:B:DB$ LoseLife | LifeAmount$ 2\nSVar:C:DB$ Draw | NumCards$ 1\n"
    )
}

fn first_effect(line: &str) -> Value {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    found[0]["Spell"]["effects"][0].clone()
}

/// "Unless any player pays": the group is asked in turn order until one pays.
#[rstest]
#[case::any_player("Player", "EachPlayer")]
#[case::any_opponent("Opponent", "Opponents")]
#[case::any_opponent_long("Player.Opponent", "Opponents")]
fn a_group_may_pay(#[case] payer: &str, #[case] who: &str) {
    let node = first_effect(&format!(
        "GainLife | LifeAmount$ 3 | UnlessCost$ 2 | UnlessPayer$ {payer}"
    ));
    assert_eq!(
        node,
        json!({"Pay": {"who": who, "cost": "{2}", "unpaid": [{"GainLife": {"amount": 3}}]}})
    );
}

/// "Any player may sacrifice a land. If a player does": the paid branch holds the effect.
#[test]
fn a_group_paying_switches_the_branch() {
    let node = first_effect(
        "GainLife | LifeAmount$ 3 | UnlessCost$ Sac<1/Land> | UnlessPayer$ Player | UnlessSwitched$ True",
    );
    assert_eq!(node["Pay"]["who"], json!("EachPlayer"));
    assert_eq!(node["Pay"]["paid"], json!([{"GainLife": {"amount": 3}}]));
    assert!(node["Pay"].get("unpaid").is_none());
}

#[rstest]
#[case::opponent("Opponent", "Opponents")]
#[case::player_opponent("Player.Opponent", "Opponents")]
#[case::another("Player.Other", "Opponents")]
#[case::everyone("Player", "EachPlayer")]
fn a_player_is_chosen_from_a_group(#[case] choices: &str, #[case] from: &str) {
    let node = first_effect(&format!("ChoosePlayer | Defined$ You | Choices$ {choices}"));
    assert_eq!(node["Choose"]["from"], json!(from));
    assert!(node["Choose"].get("who").is_none(), "{node}");
}

#[test]
fn no_choices_means_every_player() {
    let node = first_effect("ChoosePlayer | Defined$ You");
    assert_eq!(node["Choose"]["from"], json!("EachPlayer"));
}

#[rstest]
#[case::lowest_life("ChoosePlayer | Defined$ You | Choices$ Player.withLowestLife")]
#[case::a_property("ChoosePlayer | Defined$ You | Choices$ Player.Opponent+lifeEQX")]
fn a_player_filter_stays_a_gap(#[case] line: &str) {
    assert_eq!(gaps(&spell(line)), ["key:ChoosePlayer.Choices"]);
}

/// Two parts chosen by the controller keep the `Either` they always were; every other choice is a `OneOf`.
#[rstest]
#[case::two_by_you(
    "GenericChoice | Choices$ A,B",
    json!({"Either": {"first": [{"GainLife": {"amount": 2}}], "second": [{"LoseLife": {"amount": 2}}]}})
)]
#[case::three_by_you(
    "GenericChoice | Choices$ A,B,C",
    json!({"OneOf": {"options": [
        [{"GainLife": {"amount": 2}}], [{"LoseLife": {"amount": 2}}], [{"Draw": {}}]]}})
)]
fn a_choice_among_lists(#[case] line: &str, #[case] node: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(found[0]["Spell"]["effects"][0], node);
}

#[rstest]
#[case::one_option("GenericChoice | Choices$ A", "key:GenericChoice.Choices")]
#[case::nine_options(
    "GenericChoice | Choices$ A,B,C,A,B,C,A,B,C",
    "key:GenericChoice.Choices"
)]
#[case::each_opponent(
    "GenericChoice | Defined$ Player.Opponent | Choices$ A,B",
    "key:GenericChoice.Defined=Player.Opponent"
)]
fn a_choice_beyond_the_node_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

#[rstest]
#[case::this_turn("ChoiceRestriction$ ThisTurn", "once_each_turn")]
#[case::this_game("ChoiceRestriction$ ThisGame", "once_ever")]
#[case::repeat("CanRepeatModes$ True", "repeat")]
fn a_charm_restriction(#[case] key: &str, #[case] field: &str) {
    let node = first_effect(&format!("Charm | Choices$ A,B | {key}"));
    assert_eq!(node["Modal"][field], json!(true), "{node}");
}

#[test]
fn an_unknown_charm_restriction_is_a_gap() {
    assert_eq!(
        gaps(&spell(
            "Charm | Choices$ A,B | ChoiceRestriction$ Sometimes"
        )),
        ["key:Charm.ChoiceRestriction=Sometimes"]
    );
}

/// An object verb that is optional is the controller's choice, with or without a target.
#[rstest]
#[case::counter("PutCounter | Defined$ Self | CounterType$ P1P1 | Optional$ True")]
#[case::counter_on_a_target(
    "PutCounter | ValidTgts$ Creature | CounterType$ P1P1 | Optional$ True"
)]
#[case::animate(
    "Animate | Defined$ Self | Power$ 2 | Toughness$ 2 | Types$ Creature | Optional$ True"
)]
fn an_optional_object_verb_asks_you(#[case] line: &str) {
    let node = first_effect(line);
    assert!(node.get("May").is_some(), "{node}");
    assert!(node["May"].get("who").is_none(), "{node}");
}

/// "That player chooses": the player of the trigger, not the controller, is asked.
#[test]
fn the_triggering_player_chooses() {
    let script = "Name:T\nTypes:Creature Elf\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player | Execute$ Ch\nSVar:Ch:DB$ GenericChoice | Defined$ TriggeredPlayer | Choices$ A,B,C\nSVar:A:DB$ GainLife | LifeAmount$ 2\nSVar:B:DB$ LoseLife | LifeAmount$ 2\nSVar:C:DB$ Draw | NumCards$ 1\n";
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    let node = &found[0]["Triggered"]["effects"][0]["OneOf"];
    assert_eq!(node["who"], json!("Triggering"), "{node}");
    assert_eq!(node["options"].as_array().map(Vec::len), Some(3));
}
