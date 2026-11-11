//! Goal: the opening-hand ability with riders (Gemstone Caverns) is written as the card words it, and a near miss of
//! its condition, its counter or its exile is not; a rider the writer cannot word writes nothing.
use super::*;

const CAVERNS: &str =
    r#"{"OpeningHandWith":{"not_starting":true,"counter":{"Other":"luck"},"exile_from_hand":1}}"#;
const TEXT: &str = "If ~ is in your opening hand and you're not the starting player, you may begin the game with ~ on the battlefield with a luck counter on it. If you do, exile a card from your hand.";

fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

#[test]
fn dk26_gemstone_caverns_is_written_as_printed() {
    assert!(
        writes(CAVERNS, TEXT),
        "{:?}",
        render_variants(&ability(CAVERNS))
    );
}

#[rstest]
#[case::any_player(r#"{"OpeningHandWith":{"counter":{"Other":"luck"},"exile_from_hand":1}}"#)]
#[case::no_counter(r#"{"OpeningHandWith":{"not_starting":true,"exile_from_hand":1}}"#)]
#[case::another_counter(
    r#"{"OpeningHandWith":{"not_starting":true,"counter":{"Other":"gem"},"exile_from_hand":1}}"#
)]
#[case::no_exile(r#"{"OpeningHandWith":{"not_starting":true,"counter":{"Other":"luck"}}}"#)]
#[case::the_plain_ability(r#""OpeningHand""#)]
fn dk26_a_near_miss_is_not_the_caverns_text(#[case] json: &str) {
    assert!(!writes(json, TEXT), "{json}");
}

#[rstest]
#[case::two_exiled(
    r#"{"OpeningHandWith":{"not_starting":true,"counter":{"Other":"luck"},"exile_from_hand":2}}"#
)]
#[case::a_plus_one_counter(r#"{"OpeningHandWith":{"counter":"+1/+1"}}"#)]
#[case::no_rider(r#"{"OpeningHandWith":{}}"#)]
fn dk26_a_rider_the_writer_cannot_word_writes_nothing(#[case] json: &str) {
    assert!(render_variants(&ability(json)).is_empty(), "{json}");
}
