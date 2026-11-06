//! Goal: the faults fix pass 8 found by reading offered cards back stay closed: a search for a kind of card put on top of the library is revealed, and a printed "you may search" with no decider in the script is a gap.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
#[case::kind_on_top(
    "ChangeType$ Instant,Sorcery | Destination$ Library | LibraryPosition$ 0",
    true
)]
#[case::kind_to_hand("ChangeType$ Creature | Destination$ Hand", true)]
// Vampiric Tutor: any card put on top is not shown.
#[case::any_card_on_top("ChangeType$ Card | Destination$ Library | LibraryPosition$ 0", false)]
#[case::kind_to_battlefield("ChangeType$ Land.Basic | Destination$ Battlefield", false)]
fn a_search_for_a_kind_into_a_hidden_zone_is_revealed(#[case] tail: &str, #[case] revealed: bool) {
    let (json, found) = card(
        "Sorcery",
        &format!("A:SP$ ChangeZone | Origin$ Library | {tail}"),
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains(r#""reveal":true"#), revealed, "{json}");
}

#[rstest]
#[case::no_decider("", true)]
#[case::decider(" | OptionalDecider$ You", false)]
fn a_printed_may_search_needs_a_decider(#[case] decider: &str, #[case] gap: bool) {
    let (_, found) = card(
        "Creature Elf",
        &format!(
            "PT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self{decider} | Execute$ TrigSearch | TriggerDescription$ When CARDNAME enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.\nSVar:TrigSearch:DB$ ChangeZone | Origin$ Library | Destination$ Library | LibraryPosition$ 0 | ChangeType$ Land.Basic"
        ),
    );
    assert_eq!(
        found
            .iter()
            .any(|g| g.contains("may-search-without-decider")),
        gap,
        "{found:?}"
    );
}

#[rstest]
#[case::a_graveyard("you may exile a creature card from a graveyard", false)]
#[case::your_graveyard("you may exile a creature card from your graveyard", true)]
fn a_card_from_a_graveyard_is_from_any_graveyard(#[case] words: &str, #[case] owned: bool) {
    let (json, found) = card(
        "Creature Horror",
        &format!(
            "PT:1/1\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ TrigExile | TriggerZones$ Battlefield | TriggerDescription$ At the beginning of your upkeep, {words}.\nSVar:TrigExile:DB$ ChangeZone | ChangeType$ Creature | ChangeNum$ 1 | Origin$ Graveyard | Destination$ Exile | Optional$ True | Hidden$ True | Chooser$ You\nOracle:At the beginning of your upkeep, {words}."
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains(r#"{"OwnedBy":"You"}"#), owned, "{json}");
}

#[rstest]
#[case::tapped_in_text_not_script(
    "Then you create a tapped Treasure token for each creature your opponents control.",
    "",
    true
)]
#[case::tapped_in_both(
    "Then you create a tapped Treasure token for each creature your opponents control.",
    " | TokenTapped$ True",
    false
)]
#[case::untapped_text("Create a Treasure token.", "", false)]
fn a_tapped_token_in_the_text_needs_the_script_flag(
    #[case] oracle: &str,
    #[case] flag: &str,
    #[case] gap: bool,
) {
    let (_, found) = card(
        "Sorcery",
        &format!(
            "A:SP$ Token | TokenScript$ c_a_treasure_sac | TokenAmount$ 1{flag}\nOracle:{oracle}"
        ),
    );
    assert_eq!(
        found.iter().any(|g| g.contains("TokenTapped-missing")),
        gap,
        "{found:?}"
    );
}
