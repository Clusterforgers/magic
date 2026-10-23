//! Goal: the sentence that limits an activated ability is written as the card words it (a condition, "during your turn", "as a sorcery", "once each turn", together in either order), and another condition, timing or limit is not its text.
use super::*;

/// Whether one of the wordings of the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

/// A tap ability that draws a card with the parts that limit it.
fn draw_when(limits: &str) -> String {
    format!(r#"{{"Activated":{{"cost":["Tap"]{limits},"effects":[{{"Draw":{{}}}}]}}}}"#)
}

const BLESSING: &str = r#","condition":{"CitysBlessing":"You"}"#;
const NO_CARDS: &str = r#","condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Eq","right":0}}"#;
const SWAMP: &str = r#","condition":{"Compare":{"left":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;
const SEVEN_LANDS: &str = r#","condition":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Ge","right":7}}"#;
const YOUR_TURN: &str = r#","condition":"YourTurn""#;
const SORCERY: &str = r#","timing":"Sorcery""#;
const ONCE: &str = r#","limit":"OncePerTurn""#;

#[rstest]
#[case::arch_of_orazca(BLESSING.to_owned(), "{T}: Draw a card. Activate only if you have the city's blessing.")]
#[case::sea_gate_wreckage(NO_CARDS.to_owned(), "{T}: Draw a card. Activate only if you have no cards in hand.")]
#[case::a_swamp(SWAMP.to_owned(), "{T}: Draw a card. Activate only if you control a Swamp.")]
#[case::seven_lands(SEVEN_LANDS.to_owned(), "{T}: Draw a card. Activate only if you control seven or more lands.")]
#[case::hoofprints_of_the_stag(YOUR_TURN.to_owned(), "{T}: Draw a card. Activate only during your turn.")]
#[case::licia(format!("{YOUR_TURN}{ONCE}"), "{T}: Draw a card. Activate only during your turn and only once each turn.")]
#[case::once_then_your_turn(format!("{YOUR_TURN}{ONCE}"), "{T}: Draw a card. Activate only once each turn and only during your turn.")]
#[case::a_sorcery_with_a_condition(format!("{BLESSING}{SORCERY}"), "{T}: Draw a card. Activate only as a sorcery and only if you have the city's blessing.")]
#[case::a_condition_and_a_sorcery(format!("{BLESSING}{SORCERY}"), "{T}: Draw a card. Activate only if you have the city's blessing and only as a sorcery.")]
#[case::three_limits(format!("{BLESSING}{SORCERY}{ONCE}"), "{T}: Draw a card. Activate only as a sorcery and only once each turn and only if you have the city's blessing.")]
fn a_limit_on_activating_is_written_as_the_card_words_it(
    #[case] limits: String,
    #[case] text: &str,
) {
    let json = draw_when(&limits);
    assert!(
        writes(&json, text),
        "{text}: {:?}",
        render_variants(&ability(&json))
    );
}

#[rstest]
#[case::another_condition(BLESSING.to_owned(), "{T}: Draw a card. Activate only if you have no cards in hand.")]
#[case::a_condition_for_your_turn(YOUR_TURN.to_owned(), "{T}: Draw a card. Activate only if it's your turn.")]
#[case::the_sorcery_dropped(format!("{BLESSING}{SORCERY}"), "{T}: Draw a card. Activate only if you have the city's blessing.")]
#[case::the_limit_dropped(format!("{YOUR_TURN}{ONCE}"), "{T}: Draw a card. Activate only during your turn.")]
#[case::another_count(SEVEN_LANDS.to_owned(), "{T}: Draw a card. Activate only if you control six or more lands.")]
#[case::no_limit(
    String::new(),
    "{T}: Draw a card. Activate only if you have the city's blessing."
)]
#[case::a_limit_the_data_lacks(BLESSING.to_owned(), "{T}: Draw a card. Activate only if you have the city's blessing and only once each turn.")]
fn another_limit_is_not_the_text(#[case] limits: String, #[case] text: &str) {
    let json = draw_when(&limits);
    assert!(!writes(&json, text), "{text}");
}
