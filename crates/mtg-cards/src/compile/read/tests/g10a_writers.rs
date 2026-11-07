//! Goal: "each creature that convoked it" reads back as the creatures that convoked the spell (`Ref::Convoked`) for a counter put on them and for a connive, and the source, another counter or another group does not read back as it.
use super::*;

/// The wordings the checker accepts for the rules, for the message of a failed golden.
fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// An enters trigger that puts one counter of the kind on `on`.
fn entering(counter: &str, on: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}},"effects":[{{"PutCounters":{{"counter":{counter},"on":{on}}}}}]}}}}"#
    )
}

/// A spell that destroys its target, then `what` connives.
fn scheme(what: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Destroy":{{"what":{{"Target":0}}}}}},{{"Connive":{{"what":{what}}}}}]}}}}"#
    )
}

const P1P1: &str = r#""+1/+1""#;
const FLYING: &str = r#"{"Other":"flying"}"#;
const CONVOKED: &str = r#""Convoked""#;
const THIS: &str = r#""This""#;
const YOURS: &str = r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;
const LOXODON: &str = "When ~ enters, put a +1/+1 counter on each creature that convoked it.";
const SINGER: &str = "When ~ enters, put a flying counter on each creature that convoked it.";
const YOURS_TEXT: &str = "When ~ enters, put a +1/+1 counter on each creature you control.";
const SCHEME: &str = "Destroy target creature. Each creature that convoked ~ connives.";
const SOURCE_CONNIVES: &str = "Destroy target creature. ~ connives.";

#[rstest]
#[case::counters(entering(P1P1, CONVOKED), LOXODON)]
#[case::flying_counters(entering(FLYING, CONVOKED), SINGER)]
#[case::connive(scheme(CONVOKED), SCHEME)]
fn the_creatures_that_convoked_it_read_back(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}: {:?}", wordings(&json));
}

#[rstest]
#[case::on_the_source(entering(P1P1, THIS), LOXODON)]
#[case::on_your_creatures(entering(P1P1, YOURS), LOXODON)]
#[case::other_counter(entering(FLYING, CONVOKED), LOXODON)]
#[case::read_as_your_creatures(entering(P1P1, CONVOKED), YOURS_TEXT)]
#[case::the_source_connives(scheme(THIS), SCHEME)]
#[case::read_as_the_source_conniving(scheme(CONVOKED), SOURCE_CONNIVES)]
fn another_group_does_not_read_back_as_them(#[case] json: String, #[case] clause: &str) {
    assert!(!written(&json, clause), "{clause}: {:?}", wordings(&json));
}
