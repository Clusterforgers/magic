//! Goal: a numbered bookkeeping name ("kept2", "top3") writes like the plain name; names that would run together keep the ability unwritten, and a changed rule does not write the old text.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const OWL_TEXT: &str = "When this creature enters, look at the top four cards of your library. You may reveal an artifact or enchantment card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.";

fn owl(kept: &str, top: &str, to: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"LookAtTop":{{"as":"{top}","count":4}}}},{{"Choose":{{"as":"{kept}","count":{{"max":1,"min":0}},"from":{{"Each":[{{"In":"Library"}},{{"Is":{{"Bound":"{top}"}}}},{{"AnyOf":[{{"Type":"Artifact"}},{{"Type":"Enchantment"}}]}}]}}}}}},{{"Reveal":{{"what":{{"Bound":"{kept}"}}}}}},{{"Move":{{"to":{to},"what":{{"Bound":"{kept}"}}}}}},{{"Move":{{"to":{{"Library":{{"order":"Random","place":"Bottom"}}}},"what":{{"Bound":"{top}"}}}}}}],"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}}}}}}"#
    )
}

#[rstest]
#[case::plain("kept", "top")]
#[case::second_use("kept2", "top")]
#[case::both_numbered("kept2", "top3")]
#[case::far_number("kept12", "top")]
fn numbered_names_write_like_plain_ones(#[case] kept: &str, #[case] top: &str) {
    assert!(written(&owl(kept, top, r#""Hand""#), OWL_TEXT));
}

#[rstest]
#[case::another_destination(owl("kept2", "top", r#"{"Battlefield":{}}"#))]
#[case::to_the_graveyard(owl("kept2", "top", r#""Graveyard""#))]
fn a_changed_rule_does_not_write_the_text(#[case] json: String) {
    assert!(!written(&json, OWL_TEXT));
}

#[test]
fn names_that_would_run_together_stay_apart() {
    // "kept2" and "kept3" would both become "kept"; the ability is left numbered and nothing writes it.
    let json = owl("kept2", "top", r#""Hand""#).replace(
        r#""Reveal":{"what":{"Bound":"kept2"}}"#,
        r#""Reveal":{"what":{"Bound":"kept3"}}"#,
    );
    assert!(!written(&json, OWL_TEXT));
}
