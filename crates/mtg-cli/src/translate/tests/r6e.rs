//! Goal: the gap pack of round 6 builder e stays right: a card taken from exile is any player's unless the text says whose.
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
// Sengir Nosferatu's Bat.
#[case::an_exiled_card(
    "Return an exiled card named Sengir Nosferatu to the battlefield under its owner's control",
    false
)]
#[case::a_card_in_exile(
    "Put a creature card in exile onto the battlefield under its owner's control",
    false
)]
#[case::you_own("Return a creature card you own in exile to the battlefield", true)]
#[case::that_player_owns(
    "Put a creature card that player owns from exile onto the battlefield",
    true
)]
fn a_card_from_exile_is_anyones_unless_the_text_says_whose(
    #[case] words: &str,
    #[case] owned: bool,
) {
    let (json, found) = card(
        "Creature Bat",
        &format!(
            "PT:1/2\nA:AB$ ChangeZone | Cost$ 1 B Sac<1/CARDNAME> | ChangeType$ Creature | ChangeNum$ 1 | Origin$ Exile | Destination$ Battlefield | Hidden$ True | SpellDescription$ {words}.\nOracle:{{1}}{{B}}, Sacrifice this creature: {words}."
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains(r#"{"OwnedBy":"You"}"#), owned, "{json}");
}
