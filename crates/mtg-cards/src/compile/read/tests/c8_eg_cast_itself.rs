//! Goal: a card's permission to cast itself from its graveyard reads with and without its condition, writes back as the
//! same text, and a near-miss (another zone, another object, another player) stays unread.
use rstest::rstest;

use super::*;
use crate::refs::ZoneKind;

#[rstest]
#[case::always("You may cast this card from your graveyard.")]
#[case::as_long_as("You may cast this card from your graveyard as long as you control a Zombie.")]
#[case::gained_life("You may cast this card from your graveyard if you gained life this turn.")]
fn c8_eg_reads_and_writes_back(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, false);
    let Ability::Rule { from, .. } = read else {
        panic!("{text}: {read:?}");
    };
    assert_eq!(from, ZoneKind::Graveyard, "{text}");
}

#[rstest]
#[case::exile("You may cast this card from exile.")]
#[case::other_cards("You may cast creature cards from your graveyard.")]
#[case::opponent("Each opponent may cast this card from your graveyard.")]
fn c8_eg_near_misses_stay_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert!(
        !matches!(
            read,
            Some(Ability::Rule {
                from: ZoneKind::Graveyard,
                ..
            })
        ),
        "{text}: {read:?}"
    );
}
