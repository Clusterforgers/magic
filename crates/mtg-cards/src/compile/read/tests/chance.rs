//! Goal: a coin flip reads with a branch for each result, alone or after a cost or an event, and a flip with no branch, a flip until a loss and "whenever you win a coin flip" read too, and a die roll reads as a roll whose table the compiler fills from the rows after it; both write back as the same text, and a near-miss (a branch twice, the lose branch first, a die with another name or a body that does not read) stays with the table.
use super::*;

#[rstest]
#[case::win_only(
    "{T}: Flip a coin. If you win the flip, destroy all nonland permanents.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"FlipCoin":{"win":[{"Destroy":{"what":{"Each":[{"Not":{"Type":"Land"}}]}}}]}}]}}"#
)]
#[case::both_results(
    "{R}, {T}: Flip a coin. If you win the flip, destroy target artifact. If you lose the flip, sacrifice this creature.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{R}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"FlipCoin":{"win":[{"Destroy":{"what":{"Target":0}}}],"lose":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#
)]
#[case::lose_only_in_a_trigger(
    "When this creature enters, flip a coin. If you lose the flip, return this creature to its owner's hand.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"FlipCoin":{"lose":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#
)]
#[case::c8_vote_a_bare_flip(
    "{1}, {T}, Sacrifice another permanent: Flip a coin.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap",{"Sacrifice":{"filter":["Other"]}}],"effects":[{"FlipCoin":{}}]}}"#
)]
#[case::c8_vote_until_a_loss(
    "At the beginning of combat on your turn, flip a coin until you lose a flip.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"FlipCoin":{"until":true}}]}}"#
)]
#[case::c8_vote_you_win_a_flip(
    "Whenever you win a coin flip, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"CoinFlipWon":{"who":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::c8_vote_a_player_wins_a_flip(
    "Whenever a player wins a coin flip, draw a card.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"CoinFlipWon":{}},"effects":[{"Draw":{}}]}}"#
)]
fn a_coin_flip_reads_with_the_branch_of_each_result(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::the_lose_branch_first(
    "{T}: Flip a coin. If you lose the flip, sacrifice this creature. If you win the flip, draw a card.",
    ClauseKind::Activated
)]
#[case::a_branch_twice(
    "{T}: Flip a coin. If you win the flip, draw a card. If you win the flip, gain 1 life.",
    ClauseKind::Activated
)]
#[case::a_branch_without_an_effect(
    "{T}: Flip a coin. If you win the flip, wish for a card.",
    ClauseKind::Activated
)]
#[case::a_choice_of_the_flip(
    "{T}: Flip a coin. If you win the flip, draw a card. If you lose, gain 1 life.",
    ClauseKind::Activated
)]
#[case::a_flip_until_a_loss_with_a_branch(
    "{T}: Flip a coin until you lose a flip. If you win the flip, draw a card.",
    ClauseKind::Activated
)]
#[case::another_player_until_a_loss(
    "Whenever a player casts a spell, that player flips a coin until they lose a flip.",
    ClauseKind::Triggered
)]
#[case::two_coins(
    "{T}: Flip two coins. If you win the flip, draw a card.",
    ClauseKind::Activated
)]
fn a_flip_that_the_table_words_differently_stays_unread(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert!(clause(text, kind, false).is_none(), "{text}");
}

#[rstest]
#[case::at_the_upkeep(
    "At the beginning of your upkeep, roll a d20.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"RollDice":{"sides":20,"results":[]}}]}}"#
)]
#[case::a_d6_alone(
    "{3}, {T}: Roll a d6.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"effects":[{"RollDice":{"sides":6,"results":[]}}]}}"#
)]
fn a_die_reads_as_a_roll_without_a_table(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::a_word_for_the_die("When this creature enters, roll a die.")]
#[case::no_faces("When this creature enters, roll a d1.")]
#[case::two_dice("When this creature enters, roll two d6.")]
#[case::a_sum("When this creature enters, roll a d20 and add the number of cards in your hand.")]
fn a_roll_with_another_shape_stays_unread(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}

#[rstest]
#[case::range("1\u{2014}9 | Draw a card.", Some((1, Some(9), "Draw a card.")))]
#[case::one_total("20 | Draw a card.", Some((20, Some(20), "Draw a card.")))]
#[case::and_more("15+ | Draw a card.", Some((15, None, "Draw a card.")))]
#[case::backwards("9\u{2014}1 | Draw a card.", None)]
#[case::zero("0 | Draw a card.", None)]
#[case::a_word("Draw | a card.", None)]
#[case::no_bar("1\u{2014}9 Draw a card.", None)]
fn the_range_of_a_row_is_read_before_the_bar(
    #[case] text: &str,
    #[case] row: Option<(u32, Option<u32>, &str)>,
) {
    assert_eq!(die_row(text), row);
}
