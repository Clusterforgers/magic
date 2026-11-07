//! Goal: the faults fix pass 11 found stay closed: "put it onto the battlefield if ...; otherwise into your hand" is one branch or keeps the second move to the zone it moves from, a single card picked from a public zone is taken when one is there, and a face-up exile of the top cards reads `Reveal$` itself.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header, Oracle text and lines as compact JSON, and its gaps.
fn card(types: &str, oracle: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\nOracle:{oracle}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

/// Guidelight Pathmaker's search: the card found goes onto the battlefield if `condition`, else to the hand. `first`
/// holds the keys of the conditional move.
fn search_then(first: &str) -> String {
    format!(
        "A:SP$ ChangeZone | Origin$ Library | Destination$ Library | ChangeType$ Artifact | ChangeNum$ 1 | Shuffle$ False | RememberChanged$ True | SubAbility$ DBBattlefield\nSVar:DBBattlefield:DB$ ChangeZone | Defined$ Remembered | Origin$ Library | Destination$ Battlefield | ConditionDefined$ Remembered | ConditionPresent$ Card.cmcLE2{first} | ForgetChanged$ True | SubAbility$ DBHand\nSVar:DBHand:DB$ ChangeZone | Defined$ Remembered | Origin$ Library | Destination$ Hand | SubAbility$ DBShuffle\nSVar:DBShuffle:DB$ Shuffle | Defined$ You | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True"
    )
}

const TO_HAND: &str = r#"{"Move":{"to":"Hand","what":{"Bound":"found"}}}"#;
const KEPT_TO_HAND: &str =
    r#"{"Move":{"to":"Hand","what":{"Each":[{"Is":{"Bound":"found"}},{"In":"Library"}]}}}"#;

#[test]
fn the_move_to_hand_is_the_otherwise_of_the_battlefield_move() {
    let (json, found) = card("Sorcery", "", &search_then(""));
    assert!(found.is_empty(), "{found:?}");
    assert!(
        json.contains(&format!(r#""otherwise":[{TO_HAND}]"#)),
        "{json}"
    );
    // Nothing moves the card after the branch: the shuffle follows it.
    assert!(json.contains(r#"}}]}},{"Shuffle":{}}"#), "{json}");
}

#[test]
fn after_a_may_the_move_to_hand_keeps_to_the_library() {
    // Treasure Chest: "you may put it onto the battlefield. Otherwise, put that card into your hand": a declined move
    // still puts the card into the hand, and one that was made leaves it on the battlefield.
    let (json, found) = card("Sorcery", "", &search_then(" | Optional$ True"));
    assert!(found.is_empty(), "{found:?}");
    assert!(!json.contains("otherwise"), "{json}");
    assert!(json.contains(KEPT_TO_HAND), "{json}");
}

#[rstest]
#[case::onto_the_battlefield("Battlefield", true)]
#[case::into_the_graveyard("Graveyard", false)]
#[case::into_exile("Exile", false)]
fn only_a_name_that_arrived_keeps_to_its_zone(#[case] first: &str, #[case] kept: bool) {
    // Without the "may" the pair is a branch; with it the second move stands on its own.
    let lines = search_then(" | Optional$ True")
        .replace("Destination$ Battlefield", &format!("Destination$ {first}"));
    let (json, found) = card("Sorcery", "", &lines);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains(KEPT_TO_HAND), kept, "{json}");
    assert_eq!(json.contains(TO_HAND), !kept, "{json}");
}

#[test]
fn a_second_move_with_a_condition_of_its_own_is_no_branch() {
    // Deadbridge Chant: each move has its own condition, so neither is the other's "otherwise".
    let lines = search_then("").replace(
        "Destination$ Hand |",
        "Destination$ Hand | ConditionDefined$ Remembered | ConditionPresent$ Card.cmcGE3 |",
    );
    let (json, found) = card("Sorcery", "", &lines);
    assert!(found.is_empty(), "{found:?}");
    assert!(!json.contains("otherwise"), "{json}");
    assert!(json.contains(KEPT_TO_HAND), "{json}");
}

/// Realmbreaker's pick: a land card from target opponent's graveyard (or another public zone) onto the battlefield.
fn pick(origin: &str) -> String {
    format!(
        "A:AB$ Mill | Cost$ 2 T | NumCards$ 3 | ValidTgts$ Opponent | SubAbility$ DBPick\nSVar:DBPick:DB$ ChangeZone | Origin$ {origin} | Destination$ Battlefield | GainControl$ True | Hidden$ True | ChangeType$ Land.TargetedPlayerCtrl | Tapped$ True"
    )
}

#[rstest]
#[case::graveyard(
    "Graveyard",
    "Put a land card from their graveyard onto the battlefield.",
    1
)]
#[case::exile(
    "Exile",
    "Put a land card they own from exile onto the battlefield.",
    1
)]
#[case::up_to_one(
    "Graveyard",
    "Put up to one land card from their graveyard onto the battlefield.",
    0
)]
#[case::may(
    "Graveyard",
    "You may put a land card from their graveyard onto the battlefield.",
    0
)]
#[case::another_ability_any_number(
    "Graveyard",
    "{T}: Put a land card from their graveyard onto the battlefield.\\nSearch your library for any number of Praetor cards.",
    0
)]
fn a_card_picked_from_a_public_zone_is_taken_when_there_is_one(
    #[case] origin: &str,
    #[case] oracle: &str,
    #[case] min: u32,
) {
    // A line without its own description falls back to the whole card, which here says "any number" elsewhere.
    let (json, found) = card("Artifact", oracle, &pick(origin));
    assert!(found.is_empty(), "{found:?}");
    let optional = json.contains(r#""count":{"max":1,"min":0}"#);
    assert_eq!(optional, min == 0, "{json}");
}

#[rstest]
#[case::own_text("Put a land card from their graveyard onto the battlefield.", false)]
#[case::own_text_may(
    "You may put a land card from their graveyard onto the battlefield.",
    true
)]
fn the_printed_ability_decides_not_the_rest_of_the_card(
    #[case] said: &str,
    #[case] optional: bool,
) {
    // Realmbreaker: the other ability's "any number" does not reach the pick of this one.
    let lines = pick("Graveyard").replace(
        "NumCards$ 3 |",
        &format!("NumCards$ 3 | SpellDescription$ {said} |"),
    );
    let oracle = format!("{said}\\nSearch your library for any number of Praetor cards.");
    let (json, found) = card("Artifact", &oracle, &lines);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(
        json.contains(r#""count":{"max":1,"min":0}"#),
        optional,
        "{json}"
    );
}

#[rstest]
#[case::revealed(" | Reveal$ True")]
#[case::plain("")]
fn a_face_up_exile_of_the_top_card_reads_its_reveal(#[case] reveal: &str) {
    // Spark of Creativity: whether `Reveal$` counts as read no longer hangs on the face-down test reading it on the way.
    let (json, found) = card(
        "Sorcery",
        "",
        &format!(
            "A:SP$ Dig | Defined$ You | DigNum$ 1 | ChangeNum$ All | DestinationZone$ Exile | RememberChanged$ True{reveal}"
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert!(json.contains("ExileTop"), "{json}");
}
