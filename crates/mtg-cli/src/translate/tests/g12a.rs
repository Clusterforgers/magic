//! Goal: round 12 pack g12a reads a card that would go to a graveyard and goes into its owner's library instead
//! (shuffled in after a reveal, on top, on the bottom), a kicked permanent that enters with counters and a lasting change
//! from an `R:` line, and leaves other library parts a gap by name.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a creature with the given script lines, as compact JSON, and its gaps.
fn card(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Creature Bear\nPT:2/2\n{lines}\nOracle:x\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const ANYWHERE: &str =
    "R:Event$ Moved | Destination$ Graveyard | ValidCard$ Card.Self | ReplaceWith$ P";
const DIES: &str = "R:Event$ Moved | ActiveZones$ Battlefield | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | ReplaceWith$ P";
const DIES_PLAIN: &str = "R:Event$ Moved | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | ReplaceWith$ P";

#[rstest]
#[case::shuffled(
    ANYWHERE,
    "Hidden$ True | Origin$ All | Destination$ Library | Defined$ ReplacedCard | Reveal$ True | Shuffle$ True",
    r#"{"IntoLibrary":{"place":"Shuffled","reveal":true}},"when":{"ZoneChange":{"to":"Graveyard","what":["This"]}}"#
)]
#[case::on_top(
    DIES,
    "Origin$ Battlefield | Destination$ Library | LibraryPosition$ 0 | Defined$ ReplacedCard",
    r#"{"IntoLibrary":{"place":"Top"}},"when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":["This"]}}"#
)]
#[case::on_the_bottom(
    DIES_PLAIN,
    "Hidden$ True | Origin$ All | Destination$ Library | LibraryPosition$ -1 | Defined$ ReplacedCard",
    r#"{"IntoLibrary":{"place":"Bottom"}},"when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":["This"]}}"#
)]
fn g12a_a_card_goes_into_its_library_instead(
    #[case] line: &str,
    #[case] part: &str,
    #[case] expected: &str,
) {
    let (json, gaps) = card(&format!("{line}\nSVar:P:DB$ ChangeZone | {part}"));
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(expected), "{json}");
}

#[rstest]
// Second from the top is no place the replacement names.
#[case::second_from_top(
    "Hidden$ True | Origin$ All | Destination$ Library | LibraryPosition$ 1 | Defined$ ReplacedCard",
    "key:Replacement.LibraryPosition"
)]
#[case::shuffled_and_placed(
    "Hidden$ True | Origin$ All | Destination$ Library | LibraryPosition$ -1 | Shuffle$ True | Defined$ ReplacedCard",
    "key:Replacement.LibraryPosition"
)]
// A reveal belongs to a move into the library only.
#[case::revealed_exile(
    "Hidden$ True | Origin$ All | Destination$ Exile | Defined$ ReplacedCard | Reveal$ True",
    "replacement:Moved-not-an-exile"
)]
fn g12a_other_library_parts_stay_gaps(#[case] part: &str, #[case] gap: &str) {
    let (_, gaps) = card(&format!("{ANYWHERE}\nSVar:P:DB$ ChangeZone | {part}"));
    assert!(gaps.iter().any(|found| found == gap), "{gaps:?}");
}

#[rstest]
#[case::haste(
    "K:Kicker:2 R\nR:Event$ Moved | ValidCard$ Card.Self+kicked | Destination$ Battlefield | ReplaceWith$ C | ReplacementResult$ Updated\nSVar:C:DB$ PutCounter | ETB$ True | Defined$ Self | CounterType$ P1P1 | CounterNum$ 2 | SubAbility$ H\nSVar:H:DB$ Animate | Defined$ Self | Keywords$ Haste | Duration$ Permanent",
    true
)]
// A change that lasts until end of turn is not how the permanent enters.
#[case::until_end_of_turn(
    "K:Kicker:2 R\nR:Event$ Moved | ValidCard$ Card.Self+kicked | Destination$ Battlefield | ReplaceWith$ C | ReplacementResult$ Updated\nSVar:C:DB$ PutCounter | ETB$ True | Defined$ Self | CounterType$ P1P1 | CounterNum$ 2 | SubAbility$ H\nSVar:H:DB$ Animate | Defined$ Self | Keywords$ Haste",
    false
)]
fn g12a_a_kicked_permanent_enters_with_counters_and_a_change(
    #[case] lines: &str,
    #[case] read: bool,
) {
    let (json, gaps) = card(lines);
    if read {
        assert!(gaps.is_empty(), "{gaps:?}");
        assert!(
            json.contains(r#"{"WithCountersAnd":{"count":2,"counter":"+1/+1","mods":[{"AddKeyword":"Haste"}]}},"condition":"Kicked""#),
            "{json}"
        );
    } else {
        assert!(!gaps.is_empty(), "{json}");
    }
}

#[rstest]
#[case::untapped("Untap", r#"{"Replacement":{"by":"Untapped","when":{"Enters":{"what":[{"Type":"Land"},{"ControlledBy":"You"}]}}}}"#)]
#[case::tapped("Tap", r#"{"Replacement":{"by":"Tapped","when":{"Enters":{"what":[{"Type":"Land"},{"ControlledBy":"You"}]}}}}"#)]
fn g12a_lands_you_control_enter_untapped_or_tapped(#[case] verb: &str, #[case] expected: &str) {
    let (json, gaps) = card(&format!(
        "R:Event$ Moved | ValidCard$ Land.YouCtrl | Destination$ Battlefield | ReplaceWith$ E | ReplacementResult$ Updated | ActiveZones$ Battlefield\nSVar:E:DB$ {verb} | ETB$ True | Defined$ ReplacedCard"
    ));
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(expected), "{json}");
}
