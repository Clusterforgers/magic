//! Goal: a look at a hand, a move of cards out of a library, a random bottom order, the size of what a step remembered, and a permission that depends on a step that did not run read as the DSL words; a shape beyond them stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

fn effects(script: &str) -> Value {
    abilities(script)[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::a_targeted_player(
    "RevealHand | ValidTgts$ Player | Look$ True",
    json!([{"LookAtHand": {"of": {"Target": 0}}}])
)]
#[case::a_targeted_opponent(
    "RevealHand | ValidTgts$ Opponent | Look$ True",
    json!([{"LookAtHand": {"of": {"Target": 0}}}])
)]
#[case::every_player(
    "RevealHand | Defined$ Player | Look$ True",
    json!([{"LookAtHand": {"of": "EachPlayer"}}])
)]
fn looking_at_a_hand_is_private_and_a_reveal_is_public(
    #[case] line: &str,
    #[case] expected: Value,
) {
    let script = spell(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(effects(&script), expected);
}

#[rstest]
#[case::with_a_memory("RevealHand | ValidTgts$ Player | Look$ True | RememberRevealed$ True")]
#[case::not_true("RevealHand | ValidTgts$ Player | Look$ False")]
fn a_look_that_keeps_the_cards_stays_a_gap(#[case] line: &str) {
    assert_eq!(gaps(&spell(line)), vec!["key:RevealHand.Look".to_owned()]);
}

#[rstest]
#[case::your_library(
    "ChangeZoneAll | ChangeType$ Card.YouOwn | Origin$ Library | Destination$ Exile",
    "Library"
)]
#[case::the_graveyard_into_the_library(
    "ChangeZoneAll | ChangeType$ Card.YouOwn | Origin$ Graveyard | Destination$ Library | Shuffle$ True",
    "Shuffled"
)]
#[case::a_targeted_players_library(
    "ChangeZoneAll | ValidTgts$ Player | Origin$ Library | Destination$ Exile",
    "Library"
)]
fn a_move_of_all_the_cards_of_a_library(#[case] line: &str, #[case] word: &str) {
    let script = spell(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let written = effects(&script).to_string();
    assert!(written.contains(word), "{written}");
    assert!(written.contains("Each"), "{written}");
}

#[rstest]
#[case::from_the_library(
    "Origin$ Library | Destination$ Library | LibraryPosition$ -1 | RandomOrder$ True",
    true
)]
#[case::from_exile(
    "Origin$ Exile | Destination$ Library | LibraryPosition$ -1 | RandomOrder$ True",
    true
)]
#[case::on_top(
    "Origin$ Exile | Destination$ Library | LibraryPosition$ 0 | RandomOrder$ True",
    false
)]
fn the_rest_go_to_the_bottom_in_a_random_order(#[case] words: &str, #[case] reads: bool) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ PeekAndReveal | PeekAmount$ 3 | RememberRevealed$ True | SubAbility$ Rest\nSVar:Rest:DB$ ChangeZoneAll | ChangeType$ Card.IsRemembered | {words}\n"
    );
    if reads {
        assert_eq!(gaps(&script), Vec::<String>::new());
        let written = effects(&script).to_string();
        assert!(
            written.contains(r#""Library":{"order":"Random","place":"Bottom"}"#),
            "{written}"
        );
    } else {
        assert_eq!(gaps(&script), vec!["key:ChangeZone.RandomOrder".to_owned()]);
    }
}

#[test]
fn the_size_of_a_memory_is_how_many_objects_it_holds() {
    let script = spell(
        "Discard | Defined$ You | NumCards$ 2 | Mode$ TgtChoose | RememberDiscarded$ True | SubAbility$ Draw\nSVar:Draw:DB$ Draw | NumCards$ Y\nSVar:Y:Count$RememberedSize/Plus.1",
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let written = effects(&script).to_string();
    assert!(
        written.contains(r#""Sum":[{"Count":[{"Is":{"Bound":"discarded"}}"#),
        "{written}"
    );
}

#[rstest]
#[case::delirium("Condition$ Delirium | ")]
#[case::a_present_card("ConditionPresent$ Card.Outlaw+YouCtrl | ")]
fn a_permission_for_what_a_conditional_step_took_runs_with_the_condition(#[case] keys: &str) {
    let script = spell(&format!(
        "Dig | {keys}Defined$ You | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ Eff\nSVar:Eff:DB$ Effect | RememberObjects$ RememberedCard | StaticAbilities$ Play | ForgetOnMoved$ Exile | Duration$ UntilTheEndOfYourNextTurn | SubAbility$ Clean\nSVar:Play:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile\nSVar:Clean:DB$ Cleanup | ClearRemembered$ True"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    let all = effects(&script);
    let [only] = all.as_array().map(Vec::as_slice).unwrap_or_default() else {
        panic!("one step holds both: {all}");
    };
    let then = &only["If"]["then"];
    assert_eq!(then.as_array().map(Vec::len), Some(2), "{only}");
    assert!(then[1].get("Rule").is_some(), "{only}");
}

#[test]
fn a_step_that_tests_the_earlier_memory_and_adds_to_it_leaves_both_in_the_memory() {
    let script = spell(
        "ChangeZone | Origin$ Battlefield | Destination$ Exile | Defined$ Self | RememberChanged$ True | SubAbility$ Top\nSVar:Top:DB$ Dig | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | ConditionDefined$ Remembered | ConditionPresent$ Card.Self | SubAbility$ Eff\nSVar:Eff:DB$ Effect | RememberObjects$ RememberedCard | StaticAbilities$ Play | ForgetOnMoved$ Exile | Duration$ UntilTheEndOfYourNextTurn\nSVar:Play:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile",
    );
    assert_eq!(gaps(&script), Vec::<String>::new());
    let written = effects(&script).to_string();
    assert!(
        written.contains(r#""AnyOf":[{"Is":{"Bound":"exiled"}},{"Is":{"Bound":"exiled2"}}]"#),
        "{written}"
    );
}

#[test]
fn a_permission_after_an_unconditional_step_is_not_moved_into_a_condition() {
    let script = spell(
        "Dig | Defined$ You | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True | SubAbility$ Eff\nSVar:Eff:DB$ Effect | RememberObjects$ RememberedCard | StaticAbilities$ Play | ForgetOnMoved$ Exile | Duration$ UntilTheEndOfYourNextTurn\nSVar:Play:Mode$ Continuous | MayPlay$ True | Affected$ Card.IsRemembered | AffectedZone$ Exile",
    );
    let all = effects(&script);
    assert_eq!(all.as_array().map(Vec::len), Some(2), "{all}");
}
