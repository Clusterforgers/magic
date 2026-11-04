//! Goal: round 3 piece P2 (choices and loops): next-turn delayed triggers and the Discard modes.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn later(phase: &str) -> String {
    format!(
        "Name:T\nTypes:Instant\nA:SP$ DelayedTrigger | NextTurn$ True | Mode$ Phase | Phase$ {phase} | ValidPlayer$ Player | Execute$ Draw\nSVar:Draw:DB$ Draw | Defined$ You\n"
    )
}

/// Arcane Denial's first trigger: nobody holds priority before the upkeep begins, so the next upkeep is the next turn's.
#[test]
fn a_next_turn_upkeep_is_the_next_upkeep() {
    let found = abilities(&later("Upkeep"));
    assert_eq!(
        found[0]["Spell"]["effects"],
        json!([{"DelayedTrigger": {"when": {"StepBegins": {"step": "Upkeep"}},
            "effects": [{"Draw": {}}]}}])
    );
}

/// A later step could still come this turn, so it stays a gap by name.
#[rstest]
#[case::draw("Draw")]
#[case::end("End of Turn")]
#[case::main("Main1")]
fn a_next_turn_later_step_is_a_gap(#[case] phase: &str) {
    assert_eq!(gaps(&later(phase)), ["key:DelayedTrigger.NextTurn"]);
}

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

/// The modes of `Discard`: the whole hand counts one player's hand, an optional discard asks the discarding player.
#[rstest]
#[case::whole_hand(
    "Discard | Mode$ Hand | Defined$ You",
    json!([{"Discard": {"count": {"Count": [{"In": "Hand"}, {"OwnedBy": "You"}]}}}])
)]
#[case::whole_hand_of_a_target(
    "Discard | Mode$ Hand | ValidTgts$ Player",
    json!([{"Discard": {"who": {"Target": 0}, "count": {"Count": [{"In": "Hand"}, {"OwnedBy": {"Target": 0}}]}}}])
)]
#[case::optional(
    "Discard | Optional$ True | NumCards$ 1",
    json!([{"May": {"effects": [{"Discard": {}}]}}])
)]
fn a_discard_mode_is_read(#[case] line: &str, #[case] effects: serde_json::Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(found[0]["Spell"]["effects"], effects);
}

/// A mode the engine does not run stays a gap named after it.
#[rstest]
#[case::reveal_you_choose(
    "Discard | ValidTgts$ Opponent | Mode$ RevealYouChoose",
    "key:Discard.Mode=RevealYouChoose"
)]
#[case::look_you_choose(
    "Discard | ValidTgts$ Opponent | Mode$ LookYouChoose",
    "key:Discard.Mode=LookYouChoose"
)]
#[case::reveal_all(
    "Discard | Mode$ RevealDiscardAll",
    "key:Discard.Mode=RevealDiscardAll"
)]
#[case::hand_of_everyone(
    "Discard | Mode$ Hand | Defined$ Player",
    "key:Discard.Mode=Hand+group"
)]
#[case::hand_with_a_filter(
    "Discard | Mode$ Hand | DiscardValid$ Creature",
    "key:Discard.Mode=Hand+count"
)]
#[case::optional_for_an_opponent(
    "Discard | Optional$ True | Defined$ Player.Opponent",
    "key:May.Optional-decider"
)]
fn an_unread_discard_mode_is_a_named_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert_eq!(gaps(&script), [gap]);
}

/// Only the upkeep of every player is the next turn's: "your" upkeep can be a turn later, so it stays a gap.
#[test]
fn a_next_turn_upkeep_of_one_player_is_a_gap() {
    let script = later("Upkeep").replace("ValidPlayer$ Player", "ValidPlayer$ You");
    assert_eq!(gaps(&script), ["key:DelayedTrigger.NextTurn"]);
}

fn choosing(choice: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChooseNumber | {choice} | SubAbility$ Pay\nSVar:Pay:DB$ LoseLife | LifeAmount$ N\nSVar:N:Count$ChosenNumber\n"
    )
}

/// `ChooseNumber` binds a number from the minimum to the maximum and the next lines read it back.
#[rstest]
#[case::default_min("Max$ 5", json!([0, 5]))]
#[case::own_min("Min$ 1 | Max$ 3", json!([1, 3]))]
#[case::life_total("Max$ LifeTotal | ListTitle$ amount of life", json!([0, {"Life": "You"}]))]
fn a_chosen_number_is_bound_and_read(#[case] choice: &str, #[case] range: serde_json::Value) {
    let script = choosing(choice).replace("LifeTotal", "Count$YourLifeTotal");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    let bind = &effects[0]["ChooseValue"]["as"];
    assert_eq!(effects[0]["ChooseValue"]["kind"], json!({"Number": range}));
    assert_eq!(
        effects[1],
        json!({"LoseLife": {"amount": {"ChosenAs": bind}}})
    );
}

/// An open range, a secret pick and a group of choosers stay gaps named after the key.
#[rstest]
#[case::no_max("Min$ 1", "key:ChooseNumber.no-Max")]
#[case::any_number("Max$ 5 | ChooseAnyNumber$ True", "key:ChooseNumber.ChooseAnyNumber")]
#[case::secretly("Max$ 5 | Secretly$ True", "key:ChooseNumber.Secretly")]
#[case::everyone("Max$ 5 | Defined$ Player", "key:ChooseNumber.Defined=group")]
fn an_unread_number_choice_is_a_named_gap(#[case] choice: &str, #[case] gap: &str) {
    assert_eq!(gaps(&choosing(choice)), [gap]);
}
