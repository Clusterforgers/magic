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

/// `AddPhase` adds the phases the card names, after the current one.
#[rstest]
#[case::combat_after_combat("ExtraPhase$ Combat | AfterPhase$ EndCombat", json!(["Combat"]))]
#[case::combat_and_main("ExtraPhase$ Combat | FollowedBy$ Main2", json!(["Combat", "Main"]))]
#[case::beginning("ExtraPhase$ Beginning", json!(["Beginning"]))]
fn extra_phases_are_added(#[case] line: &str, #[case] phases: serde_json::Value) {
    let script = spell(&format!("AddPhase | {line}"));
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        found[0]["Spell"]["effects"],
        json!([{"AddPhase": {"phases": phases}}])
    );
}

/// A phase the engine does not add stays a gap named after it.
#[rstest]
#[case::upkeep("ExtraPhase$ Upkeep", "key:AddPhase.Upkeep")]
#[case::combat_after_main("ExtraPhase$ Combat | AfterPhase$ Main1", "key:AddPhase.CombatMain1")]
#[case::delayed(
    "ExtraPhase$ Combat | AfterPhase$ EndCombat | ExtraPhaseDelayedTrigger$ T",
    "key:AddPhase.ExtraPhaseDelayedTrigger"
)]
fn an_unread_extra_phase_is_a_named_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(&format!("AddPhase | {line}"))), [gap]);
}

/// A flip or a choice that names a target declares it for the parts to read.
#[rstest]
#[case::flip(
    "A:SP$ FlipCoin | ValidTgts$ Creature | WinSubAbility$ Win | LoseSubAbility$ Lose",
    "FlipCoin"
)]
#[case::choice(
    "A:SP$ GenericChoice | ValidTgts$ Creature | Defined$ You | Choices$ Win,Lose",
    "Either"
)]
fn a_target_of_a_flip_or_choice_is_read_by_its_parts(#[case] head: &str, #[case] node: &str) {
    let script = format!(
        "Name:T\nTypes:Sorcery\n{head}\nSVar:Win:DB$ Destroy | Defined$ Targeted\nSVar:Lose:DB$ Tap | Defined$ Targeted\n"
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        found[0]["Spell"]["targets"].as_array().map(Vec::len),
        Some(1)
    );
    assert!(
        found[0]["Spell"]["effects"][0][node]
            .to_string()
            .contains(r#"{"Target":0}"#)
    );
}

/// The target of a choice may be the chooser in Forge; without `Defined$ You` that is not read.
#[test]
fn a_choice_by_its_target_is_a_gap() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ GenericChoice | ValidTgts$ Opponent | Choices$ Win,Lose\nSVar:Win:DB$ Draw\nSVar:Lose:DB$ Draw\n";
    assert_eq!(gaps(script), ["key:GenericChoice.ValidTgts-chooser"]);
}

/// `RepeatEach` over cards runs the part for each card that matches, bound as `Remembered`.
#[test]
fn a_repeat_over_cards_runs_for_each_card() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ RepeatEach | RepeatCards$ Creature.OppCtrl | Zone$ Battlefield | RepeatSubAbility$ Hit\nSVar:Hit:DB$ Tap | Defined$ Remembered\n";
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    let each = &found[0]["Spell"]["effects"][0]["RepeatEach"];
    assert!(each["over"]["Each"].is_array(), "{each}");
    assert_eq!(
        each["effects"][0],
        json!({"Tap": {"what": {"Bound": each["as"]}}})
    );
}

#[test]
fn a_repeat_over_players_and_cards_is_a_gap() {
    let script = "Name:T\nTypes:Sorcery\nA:SP$ RepeatEach | RepeatCards$ Creature | RepeatPlayers$ Player | RepeatSubAbility$ Hit\nSVar:Hit:DB$ Tap | Defined$ Remembered\n";
    assert_eq!(gaps(script), ["key:RepeatEach.RepeatPlayers+RepeatCards"]);
}

/// A revealed choice reveals the cards chosen.
#[test]
fn a_revealed_card_choice_reveals_the_choice() {
    let script = spell("ChooseCard | Choices$ Creature | Reveal$ True");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(
        effects[1],
        json!({"Reveal": {"what": {"Bound": effects[0]["Choose"]["as"]}}})
    );
}
