//! Goal: "whenever you commit a crime" and "whenever an opponent commits a crime" read into the DSL before the effects of the card, with a limit of once each turn, and write back as the card's own text; another verb or object stays with the table.
use super::*;

#[rstest]
#[case::a_counter_once_each_turn(
    "Whenever you commit a crime, put a loot counter on ~. This ability triggers only once each turn.",
    r#"{"Triggered":{"when":{"W11cCrime":{"who":"You"}},"limit":"OncePerTurn","effects":[{"PutCounters":{"on":"This","counter":{"Other":"loot"}}}]}}"#
)]
#[case::an_opponent_proliferate(
    "Whenever an opponent commits a crime, proliferate.",
    r#"{"Triggered":{"when":{"W11cCrime":{"who":"Opponents"}},"effects":[{"Proliferate":{}}]}}"#
)]
#[case::you_draw(
    "Whenever you commit a crime, draw a card.",
    r#"{"Triggered":{"when":{"W11cCrime":{"who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
fn w11c_crime_wordings_read_and_write_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::no_object("Whenever you commit, draw a card.")]
#[case::two_crimes("Whenever you commit two crimes, draw a card.")]
fn w11c_crime_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
