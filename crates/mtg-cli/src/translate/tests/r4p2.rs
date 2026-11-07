//! Goal: round 4 piece P2 (choices and memory): `Repeat` with a fixed number of passes and no other end.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Repeat | {line}\nSVar:Body:DB$ GainLife | LifeAmount$ 2\nSVar:X:Count$xPaid\n"
    )
}

/// A fixed count is the bound of the loop, an amount is a second bound; both end the loop.
#[rstest]
#[case::literal("MaxRepeat$ 2 | RepeatSubAbility$ Body", json!({"most": 2}))]
#[case::amount("MaxRepeat$ X | RepeatSubAbility$ Body", json!({"most": 1000, "times": "X"}))]
fn a_counted_repeat_is_a_bounded_loop(#[case] line: &str, #[case] bound: serde_json::Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let mut want = json!({"condition": {"Compare": {"left": 0, "op": "Eq", "right": 0}}, "effects": [{"GainLife": {"amount": 2}}]});
    for (key, value) in bound.as_object().into_iter().flatten() {
        want[key] = value.clone();
    }
    assert_eq!(found[0]["Spell"]["effects"], json!([{"RepeatWhile": want}]));
}

/// Any other end of a repeat, or none, stays a gap named after the key.
#[rstest]
#[case::optional(
    "RepeatSubAbility$ Body | RepeatOptional$ True",
    "key:Repeat.RepeatOptional"
)]
#[case::check(
    "RepeatSubAbility$ Body | RepeatCheckSVar$ X | RepeatSVarCompare$ EQ0",
    "key:Repeat.RepeatCheckSVar"
)]
#[case::present(
    "RepeatSubAbility$ Body | RepeatPresent$ Card | RepeatDefined$ Remembered",
    "key:Repeat.RepeatPresent"
)]
#[case::no_end("RepeatSubAbility$ Body", "key:Repeat.no-MaxRepeat")]
#[case::zero(
    "MaxRepeat$ 0 | RepeatSubAbility$ Body",
    "key:Repeat.MaxRepeat=out-of-range"
)]
fn a_repeat_without_a_defined_end_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

fn roll(line: &str, after: &str) -> String {
    format!(
        "Name:T\nTypes:Creature\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Roll | TriggerDescription$ x\nSVar:Roll:DB$ RollDice | {line} | SubAbility$ Next\nSVar:Next:{after}\nSVar:X:Count$xPaid\n"
    )
}

/// The result of a roll read as a number is the die result, in the one row that holds the lines after the roll; a name
/// other than the roll's own stays what it was.
#[rstest]
#[case::result("ResultSVar$ Result", "DB$ GainLife | LifeAmount$ Result", json!({"DieResult": "Only"}))]
#[case::x("Sides$ 20 | ResultSVar$ X", "DB$ GainLife | LifeAmount$ X", json!({"DieResult": "Only"}))]
#[case::other_name("ResultSVar$ Result", "DB$ GainLife | LifeAmount$ X", json!("X"))]
fn a_roll_read_as_a_number_holds_the_lines_after_it(
    #[case] line: &str,
    #[case] after: &str,
    #[case] amount: serde_json::Value,
) {
    let script = roll(line, after);
    let sides = if line.contains("Sides$ 20") { 20 } else { 6 };
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(
        found[0]["Triggered"]["effects"],
        json!([{"RollDice": {"sides": sides, "results": [
            {"from": 1, "effects": [{"GainLife": {"amount": amount}}]}]}}])
    );
}

/// Several dice have no single result: still a gap.
#[test]
fn two_dice_with_a_result_name_stay_a_gap() {
    let script = roll(
        "Amount$ 2 | ResultSVar$ Result",
        "DB$ GainLife | LifeAmount$ Result",
    );
    assert_eq!(gaps(&script), ["key:RollDice.Amount"]);
}

fn choose(extra: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {extra}\nSVar:Pick:DB$ ChooseCard | Choices$ Creature.YouCtrl | RememberChosen$ True | Mandatory$ True | SubAbility$ Hit\nSVar:Hit:DB$ Destroy | Defined$ Remembered\nSVar:Loop:DB$ RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ Pick\n"
    )
}

/// A chosen card that is remembered is what the later lines name as remembered; inside a pass of a loop, where the
/// remembered player is already bound, it stays a gap.
#[test]
fn a_remembered_choice_is_the_object_the_next_line_names() {
    let script = choose(
        "ChooseCard | Choices$ Creature.YouCtrl | RememberChosen$ True | Mandatory$ True | SubAbility$ Hit",
    );
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    let bound = effects[0]["Choose"]["as"].clone();
    assert_eq!(effects[1], json!({"Destroy": {"what": {"Bound": bound}}}));
}

#[test]
fn a_remembered_choice_in_a_pass_over_players_is_a_gap() {
    let script = choose("RepeatEach | RepeatPlayers$ Player | RepeatSubAbility$ Pick");
    assert_eq!(
        gaps(&script),
        ["key:ChooseCard.RememberChosen-with-remembered"]
    );
}

/// Looking at the top cards of a library and putting them back in an order: the library is the line's player's.
#[rstest]
#[case::own("Defined$ You | NumCards$ 3", json!("You"), json!(3), json!([]))]
#[case::targeted("ValidTgts$ Player | NumCards$ 1", json!({"Target": 0}), json!(1), json!([{"what": "Player"}]))]
fn the_top_of_a_library_is_put_back_in_a_chosen_order(
    #[case] line: &str,
    #[case] who: serde_json::Value,
    #[case] count: serde_json::Value,
    #[case] targets: serde_json::Value,
) {
    let script = format!("Name:T\nTypes:Sorcery\nA:SP$ RearrangeTopOfLibrary | {line}\n");
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let effects = &found[0]["Spell"]["effects"];
    let top = effects[0]["LookAtTop"]["as"].clone();
    assert_eq!(
        effects[0]["LookAtTop"]
            .get("count")
            .cloned()
            .unwrap_or(json!(1)),
        count
    );
    assert_eq!(
        effects[0]["LookAtTop"]
            .get("who")
            .cloned()
            .unwrap_or(json!("You")),
        who
    );
    assert_eq!(
        effects[1],
        json!({"Move": {"what": {"Bound": top}, "to": {"Library": {"place": "Top", "order": "Chosen"}}}})
    );
    if !targets.as_array().is_some_and(Vec::is_empty) {
        assert_eq!(found[0]["Spell"]["targets"], targets);
    }
}

#[rstest]
#[case::shuffle_word(
    "Defined$ You | NumCards$ 3 | MayShuffle$ Sometimes",
    "key:RearrangeTopOfLibrary.MayShuffle"
)]
#[case::other_orderer(
    "Defined$ You | RearrangePlayer$ Targeted | NumCards$ 3",
    "key:RearrangeTopOfLibrary.RearrangePlayer"
)]
fn a_shuffle_option_or_another_orderer_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = format!("Name:T\nTypes:Sorcery\nA:SP$ RearrangeTopOfLibrary | {line}\n");
    assert_eq!(gaps(&script), [gap]);
}
