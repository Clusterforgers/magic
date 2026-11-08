//! Goal: a token replacement that keeps the tokens and adds more ("those tokens plus that many Squirrels", "plus an
//! additional Treasure token") reads into `ReplaceBy::Plus` in both wordings, writes back as the same text, and every
//! near miss (another count, kind, controller or token) reads `Differs`.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const SQUIRRELS: &str = r#"{"Replacement":{"when":{"TokenCreation":{}},"by":{"Plus":[{"CreateToken":{"token":{"Custom":{"name":"Squirrel","colors":["Green"],"types":["Creature"],"subtypes":["Squirrel"],"power":1,"toughness":1}},"count":"Triggering"}}]}}}"#;
const FOOD: &str = r#"{"Replacement":{"when":{"TokenCreation":{}},"by":{"Plus":[{"CreateToken":{"token":{"Named":"food"}}}]}}}"#;
const TREASURE: &str = r#"{"Replacement":{"when":{"TokenCreation":{"what":[{"Subtype":"Treasure"}]}},"by":{"Plus":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}}"#;
const SOLDIER: &str = r#"{"Replacement":{"when":{"TokenCreation":{"what":[{"Type":"Creature"}]}},"by":{"Plus":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}}}]}}}"#;

const CHATTERFANG: &str = "If one or more tokens would be created under your control, those tokens plus that many 1/1 green Squirrel creature tokens are created instead.";
const PEREGRIN: &str = "If one or more tokens would be created under your control, those tokens plus an additional Food token are created instead.";
const TIPPY: &str = "If you would create one or more tokens, instead create those tokens plus an additional Food token.";
const XORN: &str = "If you would create one or more Treasure tokens, instead create those tokens plus an additional Treasure token.";
const ALLENAL: &str = "If one or more creature tokens would be created under your control, those tokens plus a 1/1 white Soldier creature token are created instead.";

#[rstest]
#[case::chatterfang(CHATTERFANG, SQUIRRELS)]
#[case::peregrin(PEREGRIN, FOOD)]
#[case::tippy_toe(TIPPY, FOOD)]
#[case::xorn(XORN, TREASURE)]
#[case::queen_allenal(ALLENAL, SOLDIER)]
fn more_tokens_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
    assert!(written(json, text), "{text}");
}

#[rstest]
#[case::one_squirrel_not_that_many(
    SQUIRRELS.replace(r#","count":"Triggering""#, ""),
    CHATTERFANG
)]
#[case::that_many_food_not_one(
    FOOD.replace(r#"{"Named":"food"}"#, r#"{"Named":"food"},"count":"Triggering""#),
    PEREGRIN
)]
#[case::another_token(FOOD.replace("food", "clue"), TIPPY)]
#[case::any_token_not_treasure(FOOD.replace("food", "treasure"), XORN)]
#[case::artifact_not_creature(SOLDIER.replace(r#"{"Type":"Creature"}]"#, r#"{"Type":"Artifact"}]"#), ALLENAL)]
#[case::an_opponents_tokens(FOOD.replace(r#""TokenCreation":{}"#, r#""TokenCreation":{"controller":"Opponents"}"#), PEREGRIN)]
#[case::instead_not_plus(FOOD.replace("Plus", "Instead"), PEREGRIN)]
fn near_misses_read_differs(#[case] json: String, #[case] text: &str) {
    assert!(!written(&json, text), "{json}");
}

const MANUFACTOR: &str =
    "If you would create a Clue, Food, or Treasure token, instead create one of each.";
const EACH: &str = r#"{"Replacement":{"when":{"TokenCreation":{"what":[{"AnyOf":[{"Subtype":"Clue"},{"Subtype":"Food"},{"Subtype":"Treasure"}]}]}},"by":{"Instead":[{"CreateToken":{"token":{"Named":"clue"},"count":"Triggering"}},{"CreateToken":{"token":{"Named":"food"},"count":"Triggering"}},{"CreateToken":{"token":{"Named":"treasure"},"count":"Triggering"}}]}}}"#;

#[test]
fn one_of_each_reads_and_writes_back() {
    reads_as(MANUFACTOR, ClauseKind::Static, false, EACH);
}

#[rstest]
#[case::one_each_not_that_many(EACH.replacen(r#","count":"Triggering""#, "", 1))]
#[case::another_kind_made(EACH.replace(r#"{"Named":"food"}"#, r#"{"Named":"blood"}"#))]
#[case::two_kinds_named(EACH.replace(r#",{"Subtype":"Treasure"}"#, ""))]
#[case::plus_not_instead(EACH.replace("Instead", "Plus"))]
fn one_of_each_near_misses_read_differs(#[case] json: String) {
    assert!(!written(&json, MANUFACTOR), "{json}");
}
