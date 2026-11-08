//! Goal: tokens made for you and for a player the spell names are written as one sentence, "you and target opponent each create ...", and a different token or owner is not written that way.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn each_create(mine: &str, theirs: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"Opponent"}}],"effects":[{{"CreateToken":{{"token":{mine}}}}},{{"CreateToken":{{"who":{{"Target":0}},"token":{theirs}}}}}]}}}}"#
    )
}

const TREASURE: &str = r#"{"Named":"treasure"}"#;
const FOOD: &str = r#"{"Named":"food"}"#;

#[test]
fn the_same_token_for_you_and_the_target_is_one_sentence() {
    assert!(written(
        &each_create(TREASURE, TREASURE),
        "You and target opponent each create a Treasure token."
    ));
}

#[test]
fn a_different_token_is_not_written_as_each() {
    assert!(!written(
        &each_create(TREASURE, FOOD),
        "You and target opponent each create a Treasure token."
    ));
}
