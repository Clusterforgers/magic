//! Goal: the r118-01 wordings read and write back: the beginning of combat said "combat on each player's turn", a player's
//! ban on attacking in a step that player may take and the number of counters an intervening "if" asks for in words;
//! a near miss stays with the table or is not written.
use super::*;

#[rstest]
#[case::each_players_combat(
    ClauseKind::Triggered,
    "At the beginning of combat on each player's turn, that player draws a card."
)]
#[case::each_combat(
    ClauseKind::Triggered,
    "At the beginning of each combat, that player draws a card."
)]
fn r118_01_wordings_read_and_write_back(#[case] kind: ClauseKind, #[case] text: &str) {
    let read = clause(text, kind, false).unwrap_or_else(|| panic!("not read: {text}"));
    let written = written_variants(&read);
    assert!(
        written.iter().any(|one| same_text(one, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::each_players_upkeep_combat(
    ClauseKind::Triggered,
    "At the beginning of combat on each player's upkeep, that player draws a card."
)]
#[case::combat_on_each_turn(
    ClauseKind::Triggered,
    "At the beginning of combat on each turn, that player draws a card."
)]
fn r118_01_near_misses_stay_with_the_table(#[case] kind: ClauseKind, #[case] text: &str) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}

const PEACE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat"}},"effects":[{"May":{"who":"Triggering","effects":[{"Draw":{"who":"Triggering"}}]}},{"If":{"condition":"Performed","then":[{"Rule":{"rule":{"Cant":{"who":"Triggering","action":{"R118Attack":{}}}},"duration":"UntilEndOfTurn"}},{"PutCounters":{"on":"This","counter":{"Other":"peace"}}}]}}]}}"#;
const PEACE_TEXT: &str = "At the beginning of combat on each player's turn, that player may draw a card. If they do, they can't attack this turn and you put a peace counter on ~.";
const TWELVE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"peace"}}},"op":"Ge","right":12}},"effects":[{"WinGame":{}}]}}"#;
const TWELVE_TEXT: &str = "At the beginning of your upkeep, if ~ has twelve or more peace counters on it, you win the game.";

#[rstest]
#[case::may_draw(PEACE, PEACE_TEXT, true)]
#[case::anyone_out_of_reach(&PEACE.replace(r#""R118Attack":{}"#, r#""R118Attack":{"you":true}"#), PEACE_TEXT, false)]
#[case::for_their_next_turn(&PEACE.replace("UntilEndOfTurn", "TheirNextTurn"), PEACE_TEXT, false)]
#[case::must_draw(&PEACE.replace(r#"{"May":{"who":"Triggering","effects":[{"Draw":{"who":"Triggering"}}]}}"#, r#"{"Draw":{"who":"Triggering"}}"#), PEACE_TEXT, false)]
#[case::if_they_dont(&PEACE.replace(r#""condition":"Performed""#, r#""condition":{"Not":"Performed"}"#), PEACE_TEXT, false)]
#[case::twelve(TWELVE, TWELVE_TEXT, true)]
#[case::eleven(&TWELVE.replace(r#""right":12"#, r#""right":11"#), TWELVE_TEXT, false)]
fn r118_01_whole_abilities_are_written(
    #[case] json: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let written = written_variants(&ability(json));
    assert_eq!(
        written.iter().any(|one| same_text(one, text)),
        reads,
        "{text}: {written:?}"
    );
}
