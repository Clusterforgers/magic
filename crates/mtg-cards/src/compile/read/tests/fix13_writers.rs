//! Goal: an activation limit with a condition writes back as its own words ("only once" for once a game, "only once each turn" for once a turn) and reads back the same, and a wrong limit reads `Differs`.
use super::*;

/// Whether the ability writes the clause back, among its wordings.
fn writes(json: &str, clause: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, clause))
}

/// The condition the translator writes for "if ~ is blue".
const BLUE: &str = r#"{"Compare":{"left":{"Count":["This",{"Color":"Blue"}]},"op":"Ge","right":1}}"#;
/// The condition the reader reads for it.
const IS_BLUE: &str = r#"{"Matches":{"subject":"This","selector":[{"Color":"Blue"}]}}"#;

/// The Surge Engine ability with a condition and a limit.
fn draw_if(condition: &str, limit: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":"{{4}}{{U}}{{U}}","condition":{condition},"limit":{limit},"effects":[{{"Draw":{{"count":3}}}}]}}}}"#
    )
}

/// The Surge Engine ability as the translator writes it.
fn blue_draw(limit: &str) -> String {
    draw_if(BLUE, limit)
}

#[test]
fn fix13_surge_engine_writes_back() {
    assert!(writes(
        &blue_draw(r#""OncePerGame""#),
        "{4}{U}{U}: Draw three cards. Activate only if ~ is blue and only once."
    ));
}

#[rstest]
#[case::once_a_game(
    "{4}{U}{U}: Draw three cards. Activate only if ~ is blue and only once.",
    r#""OncePerGame""#
)]
#[case::once_a_game_first(
    "{4}{U}{U}: Draw three cards. Activate only once and only if ~ is blue.",
    r#""OncePerGame""#
)]
#[case::once_each_turn(
    "{4}{U}{U}: Draw three cards. Activate only if ~ is blue and only once each turn.",
    r#""OncePerTurn""#
)]
fn fix13_the_limit_beside_a_condition_reads_back(#[case] text: &str, #[case] limit: &str) {
    reads_as(text, ClauseKind::Activated, false, &draw_if(IS_BLUE, limit));
}

#[rstest]
#[case::once_a_game_is_not_each_turn(
    r#""OncePerGame""#,
    "{4}{U}{U}: Draw three cards. Activate only if ~ is blue and only once each turn."
)]
#[case::each_turn_is_not_once_a_game(
    r#""OncePerTurn""#,
    "{4}{U}{U}: Draw three cards. Activate only if ~ is blue and only once."
)]
#[case::twice_is_not_once(
    r#"{"TimesPerTurn":2}"#,
    "{4}{U}{U}: Draw three cards. Activate only if ~ is blue and only once each turn."
)]
fn fix13_a_wrong_limit_differs(#[case] limit: &str, #[case] clause: &str) {
    assert!(!writes(&blue_draw(limit), clause), "{limit}: {clause}");
}
