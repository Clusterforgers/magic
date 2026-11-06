//! Goal: a card that can be a commander carries the rule the deck check reads, and a deck-building line that changes nothing in play adds none.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{abilities, gaps};

#[test]
fn can_be_your_commander_is_the_rule_the_deck_check_reads() {
    let text = "Name:Test Walker\nTypes:Legendary Planeswalker Test\nManaCost:2 G\nLoyalty:4\nK:CARDNAME can be your commander.\n";
    assert_eq!(gaps(text), Vec::<String>::new());
    assert!(abilities(text).contains(&json!({"Rule": {"rule": "CanBeCommander"}})));
}
