//! Goal: the r61-00 noun suffix "that fought this turn" reads as the turn's fights and writes back, in a condition of an
//! activated ability and a target, and a near miss (another time, another verb, another controller) does not read as it.
use super::*;

const RING: &str = r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"DidThisTurn":{"event":"Fought"}},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}"#;
const RING_TEXT: &str =
    "{T}: Create a Treasure token. Activate only if you control a creature that fought this turn.";

#[rstest]
#[case::as_printed(RING_TEXT, true)]
#[case::last_turn(&RING_TEXT.replace("this turn", "last turn"), false)]
#[case::attacked(&RING_TEXT.replace("fought", "attacked"), false)]
#[case::opponent(&RING_TEXT.replace("you control", "an opponent controls"), false)]
fn r6100_a_creature_that_fought_this_turn_reads(#[case] text: &str, #[case] same: bool) {
    let read = clause(text, ClauseKind::Activated, false);
    assert_eq!(read == Some(ability(RING)), same, "{text}: {read:?}");
}

#[test]
fn r6100_a_creature_that_fought_this_turn_writes_back() {
    assert!(
        written_variants(&ability(RING))
            .iter()
            .any(|written| same_text(written, RING_TEXT))
    );
}

#[rstest]
#[case::fought("Fought", true)]
#[case::blocked("Blocked", false)]
#[case::attacked("Attacked", false)]
fn r6100_only_fights_write_as_fought(#[case] event: &str, #[case] fought: bool) {
    let json = RING.replace("Fought", event);
    let written = written_variants(&ability(&json));
    assert_eq!(
        written.iter().any(|w| w.contains("fought this turn")),
        fought,
        "{written:?}"
    );
}
