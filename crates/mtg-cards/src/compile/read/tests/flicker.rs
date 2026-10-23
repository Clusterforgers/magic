//! Goal: "Exile ~, then return it to the battlefield [transformed] under your control." reads as an exile that binds the card and a move of it onto the battlefield (with the back face up when the sentence says transformed), as a spell sentence and as a Saga chapter, writes back as the same text, and a wording that differs stays with the table.
use super::*;

use crate::compile::read::chapter_block;

/// The two effects as data: the card exiled and bound, and the move of it onto the battlefield.
fn the_return(battlefield: &str) -> String {
    format!(
        r#"[{{"Exile":{{"what":"This","as":"exiled"}}}},{{"Move":{{"what":{{"Bound":"exiled"}},"to":{{"Battlefield":{battlefield}}}}}}}]"#
    )
}

#[rstest]
#[case::transformed_under_you(
    "Exile ~, then return it to the battlefield transformed under your control.",
    r#"{"under":"You","transformed":true}"#
)]
#[case::under_you(
    "Exile ~, then return it to the battlefield under your control.",
    r#"{"under":"You"}"#
)]
#[case::under_its_owner(
    "Exile ~, then return it to the battlefield transformed under its owner's control.",
    r#"{"transformed":true}"#
)]
fn the_card_exiled_and_returned_reads_and_writes_back(
    #[case] text: &str,
    #[case] battlefield: &str,
) {
    let json = format!(r#"{{"Spell":{{"effects":{}}}}}"#, the_return(battlefield));
    reads_as(text, ClauseKind::Spell, false, &json);
}

#[test]
fn the_last_chapter_of_a_transforming_saga_exiles_it_and_returns_it_transformed() {
    let line =
        "III \u{2014} Exile ~, then return it to the battlefield transformed under your control.";
    let Some(abilities) = chapter_block(line, false, &Own::default()) else {
        panic!("not read: {line}");
    };
    let json = format!(
        r#"{{"Triggered":{{"when":{{"Chapter":{{"number":3}}}},"effects":{}}}}}"#,
        the_return(r#"{"under":"You","transformed":true}"#)
    );
    assert_eq!(abilities, vec![ability(&json)]);
    let written = render_variants(&abilities[0]);
    assert!(
        written.iter().any(|text| same_text(text, line)),
        "{written:?}"
    );
}

#[rstest]
#[case::another_card(
    "Exile target creature, then return it to the battlefield under your control."
)]
#[case::to_the_hand("Exile ~, then return it to its owner's hand.")]
#[case::no_return("Exile ~, then draw a card.")]
#[case::tapped("Exile ~, then return it to the battlefield tapped under your control.")]
#[case::under_an_opponent(
    "Exile ~, then return it to the battlefield under target opponent's control."
)]
#[case::front_face_up("Exile ~, then return it to the battlefield (front face up).")]
#[case::more_after_it(
    "Exile ~, then return it to the battlefield transformed under your control and draw a card."
)]
fn a_return_that_differs_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}
