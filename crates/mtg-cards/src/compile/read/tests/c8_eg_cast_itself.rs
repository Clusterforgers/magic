//! Goal: a card's permission to cast itself from its graveyard reads with and without its condition and the costs it adds,
//! writes back as the same text, and a near-miss (another zone, another object, another player, a cost the reader does not
//! word) stays unread.
use rstest::rstest;

use super::*;
use crate::cost::{Cost, CostItem};
use crate::modify::GameRule;
use crate::refs::ZoneKind;
use crate::value::Amount;

#[rstest]
#[case::always("You may cast this card from your graveyard.")]
#[case::as_long_as("You may cast this card from your graveyard as long as you control a Zombie.")]
#[case::gained_life("You may cast this card from your graveyard if you gained life this turn.")]
#[case::life_and_a_card(
    "You may cast this card from your graveyard by paying 3 life and discarding a card in addition to paying its other costs."
)]
#[case::a_card_only(
    "You may cast this card from your graveyard by discarding a card in addition to paying its other costs."
)]
#[case::life_only(
    "You may cast this card from your graveyard by paying 2 life in addition to paying its other costs."
)]
#[case::mana_and_two_cards(
    "You may cast this card from your graveyard by paying {1} and discarding two cards in addition to paying its other costs."
)]
#[case::three_parts(
    "You may cast this card from your graveyard by paying 1 life, paying {R}, and discarding a card in addition to paying its other costs."
)]
#[case::added_costs_and_a_condition(
    "You may cast this card from your graveyard by paying 3 life in addition to paying its other costs as long as you control a Zombie."
)]
fn c8_eg_reads_and_writes_back(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, false);
    let Ability::Rule { from, .. } = read else {
        panic!("{text}: {read:?}");
    };
    assert_eq!(from, ZoneKind::Graveyard, "{text}");
}

#[rstest]
#[case::life_and_a_card(
    "You may cast this card from your graveyard by paying 3 life and discarding a card in addition to paying its other costs.",
    vec![CostItem::PayLife(Amount::Fixed(3)), CostItem::Discard { count: Amount::Fixed(1), filter: None }]
)]
#[case::two_cards(
    "You may cast this card from your graveyard by discarding two cards in addition to paying its other costs.",
    vec![CostItem::Discard { count: Amount::Fixed(2), filter: None }]
)]
fn the_added_costs_are_the_permissions_additional_cost(
    #[case] text: &str,
    #[case] items: Vec<CostItem>,
) {
    let read = golden(text, ClauseKind::Static, false);
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        ..
    } = read
    else {
        panic!("{text}: {read:?}");
    };
    assert_eq!(permission.additional, Some(Cost(items)), "{text}");
}

#[rstest]
#[case::exile("You may cast this card from exile.")]
#[case::other_cards("You may cast creature cards from your graveyard.")]
#[case::opponent("Each opponent may cast this card from your graveyard.")]
#[case::without_the_closing_words(
    "You may cast this card from your graveyard by paying 3 life and discarding a card."
)]
#[case::an_unknown_part(
    "You may cast this card from your graveyard by sacrificing a creature in addition to paying its other costs."
)]
#[case::an_announced_amount(
    "You may cast this card from your graveyard by paying X life in addition to paying its other costs."
)]
#[case::another_card(
    "You may cast target card from your graveyard by paying 3 life in addition to paying its other costs."
)]
#[case::from_the_hand(
    "You may cast this card from your hand by paying 3 life in addition to paying its other costs."
)]
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
