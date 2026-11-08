//! Goal: "not (not a or not b)" in a list of conditions writes like "a" and "b"; any other negation, and a changed condition, stays as written and does not write the legendary-artifact text.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const TEXT: &str = "When this creature enters, look at the top four cards of your library. You may reveal a legendary artifact card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.";

fn pick(filter: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"LookAtTop":{{"as":"top","count":4}}}},{{"Choose":{{"as":"kept","count":{{"max":1,"min":0}},"from":{{"Each":[{{"In":"Library"}},{{"Is":{{"Bound":"top"}}}},{filter}]}}}}}},{{"Reveal":{{"what":{{"Bound":"kept"}}}}}},{{"Move":{{"to":"Hand","what":{{"Bound":"kept"}}}}}},{{"Move":{{"to":{{"Library":{{"order":"Random","place":"Bottom"}}}},"what":{{"Bound":"top"}}}}}}],"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}}}}}}"#
    )
}

#[rstest]
#[case::as_listed(r#"{"Type":"Artifact"},{"Supertype":"Legendary"}"#)]
#[case::negated_pair(
    r#"{"Not":{"AnyOf":[{"Not":{"Type":"Artifact"}},{"Not":{"Supertype":"Legendary"}}]}}"#
)]
fn a_double_negation_writes_as_both_conditions(#[case] filter: &str) {
    let json = pick(filter);
    assert!(
        written(&json, TEXT),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::plain_or(r#"{"AnyOf":[{"Type":"Artifact"},{"Supertype":"Legendary"}]}"#)]
#[case::one_negation_left(
    r#"{"Not":{"AnyOf":[{"Not":{"Type":"Artifact"}},{"Supertype":"Legendary"}]}}"#
)]
#[case::another_supertype(
    r#"{"Not":{"AnyOf":[{"Not":{"Type":"Artifact"}},{"Not":{"Supertype":"Snow"}}]}}"#
)]
#[case::only_artifact(r#"{"Type":"Artifact"}"#)]
fn another_condition_does_not_write_the_text(#[case] filter: &str) {
    assert!(!written(&pick(filter), TEXT));
}
