//! Goal: an exile face down that no player may look at reads back as the exile with "face down", and a face-up exile, a card held for its exiler or another count, zone or player does not read back as it.
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

/// An activated ability `{T}: <effect>`.
fn tapped(effect: &str) -> String {
    format!(r#"{{"Activated":{{"cost":["Tap"],"effects":[{effect}]}}}}"#)
}

const HAND_HELD: &str =
    r#"{"ExileHidden":{"held":true,"what":{"Each":[{"OwnedBy":"You"},{"In":"Hand"}]}}}"#;
const HAND_HIDDEN: &str = r#"{"ExileHidden":{"what":{"Each":[{"OwnedBy":"You"},{"In":"Hand"}]}}}"#;
const HAND_UP: &str = r#"{"Exile":{"what":{"Each":[{"OwnedBy":"You"},{"In":"Hand"}]}}}"#;
const LIBRARY: &str =
    r#"{"ExileHidden":{"sealed":true,"what":{"Each":[{"OwnedBy":"You"},{"In":"Library"}]}}}"#;
const HAND: &str =
    r#"{"ExileHidden":{"sealed":true,"what":{"Each":[{"OwnedBy":"You"},{"In":"Hand"}]}}}"#;
const TOP: &str = r#"{"ExileTop":{"as":"exiled","sealed":true}}"#;
const TOP_UP: &str = r#"{"ExileTop":{"as":"exiled"}}"#;
const TOP_TWO: &str = r#"{"ExileTop":{"as":"exiled","count":2,"sealed":true}}"#;

#[rstest]
#[case::hand(HAND, "{T}: Exile all cards from your hand face down.")]
#[case::top(TOP, "{T}: Exile the top card of your library face down.")]
#[case::top_two(TOP_TWO, "{T}: Exile the top two cards of your library face down.")]
fn a_sealed_exile_reads_as_the_exile_face_down(#[case] effect: &str, #[case] clause: &str) {
    let json = tapped(effect);
    assert!(written(&json, clause), "{clause}: {:?}", wordings(&json));
}

#[rstest]
#[case::face_up(HAND_UP, "{T}: Exile all cards from your hand face down.")]
#[case::held_for_the_exiler(HAND_HELD, "{T}: Exile all cards from your hand face down.")]
#[case::hidden_by_the_source(HAND_HIDDEN, "{T}: Exile all cards from your hand face down.")]
#[case::other_zone(HAND, "{T}: Exile all cards from your library face down.")]
#[case::the_library_for_the_hand(LIBRARY, "{T}: Exile all cards from your hand face down.")]
#[case::sealed_read_face_up(HAND, "{T}: Exile all cards from your hand.")]
#[case::top_face_up(TOP_UP, "{T}: Exile the top card of your library face down.")]
#[case::top_sealed_read_face_up(TOP, "{T}: Exile the top card of your library.")]
#[case::other_count(TOP_TWO, "{T}: Exile the top card of your library face down.")]
fn another_exile_does_not_read_back_as_it(#[case] effect: &str, #[case] clause: &str) {
    let json = tapped(effect);
    assert!(!written(&json, clause), "{clause}: {:?}", wordings(&json));
}
