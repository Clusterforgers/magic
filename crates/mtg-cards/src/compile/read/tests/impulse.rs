//! Goal: an impulse draw, "Exile the top card of your library. You may play that card this turn.", reads as the exile of the top cards and the permission to play them for the duration the text names (this turn, until the end of your next turn), for one card and for several, in each order of the duration and each pronoun, alone, in a trigger and in an activated ability, writes back as the same text, and leaves a pronoun that does not fit the count and every other wording to the table.
use super::*;
use crate::effect::{Effect, impulse};
use crate::modify::Duration;
use crate::value::Amount;

#[rstest]
#[case::this_turn(
    "Exile the top card of your library. You may play that card this turn.",
    1,
    Duration::UntilEndOfTurn
)]
#[case::until_end_of_turn_first(
    "Exile the top card of your library. Until end of turn, you may play that card.",
    1,
    Duration::UntilEndOfTurn
)]
#[case::until_the_end_of_your_next_turn(
    "Exile the top card of your library. Until the end of your next turn, you may play that card.",
    1,
    Duration::UntilEndOfYourNextTurn
)]
#[case::two_cards_this_turn(
    "Exile the top two cards of your library. You may play those cards this turn.",
    2,
    Duration::UntilEndOfTurn
)]
#[case::two_cards_as_them(
    "Exile the top two cards of your library. You may play them this turn.",
    2,
    Duration::UntilEndOfTurn
)]
#[case::two_cards_until_end_of_turn_first(
    "Exile the top two cards of your library. Until end of turn, you may play them.",
    2,
    Duration::UntilEndOfTurn
)]
#[case::two_cards_until_the_end_of_your_next_turn(
    "Exile the top two cards of your library. Until the end of your next turn, you may play those cards.",
    2,
    Duration::UntilEndOfYourNextTurn
)]
#[case::three_cards_as_them_next_turn(
    "Exile the top three cards of your library. Until the end of your next turn, you may play them.",
    3,
    Duration::UntilEndOfYourNextTurn
)]
fn an_impulse_draw_reads_as_its_two_effects(
    #[case] text: &str,
    #[case] count: i32,
    #[case] duration: Duration,
) {
    let expected = Ability::Spell {
        targets: Vec::new(),
        effects: impulse(Amount::Fixed(count), duration),
    };
    assert_eq!(golden(text, ClauseKind::Spell, false), expected);
}

#[test]
fn an_impulse_draw_in_a_trigger_reads_and_writes_back() {
    let read = golden(
        "When ~ enters, exile the top card of your library. Until end of turn, you may play that card.",
        ClauseKind::Triggered,
        false,
    );
    let Ability::Triggered { effects, .. } = read else {
        panic!("a triggered ability");
    };
    assert_eq!(effects, impulse(Amount::one(), Duration::UntilEndOfTurn));
}

#[test]
fn an_impulse_draw_in_an_activated_ability_reads_and_writes_back() {
    let read = golden(
        "{2}, Sacrifice ~: Exile the top card of your library. You may play that card this turn.",
        ClauseKind::Activated,
        false,
    );
    let Ability::Activated { effects, .. } = read else {
        panic!("an activated ability");
    };
    assert_eq!(effects, impulse(Amount::one(), Duration::UntilEndOfTurn));
}

#[test]
fn the_exile_alone_reads_and_writes_back() {
    let Ability::Spell { effects, .. } = golden(
        "Exile the top two cards of your library.",
        ClauseKind::Spell,
        false,
    ) else {
        panic!("a spell");
    };
    assert_eq!(
        effects,
        impulse(Amount::Fixed(2), Duration::UntilEndOfTurn)[..1]
    );
    assert!(matches!(effects[0], Effect::ExileTop { .. }));
}

#[rstest]
#[case::a_permission_with_nothing_exiled_before("You may play that card this turn.")]
#[case::them_for_one_card("Exile the top card of your library. You may play them this turn.")]
#[case::that_card_for_two(
    "Exile the top two cards of your library. You may play that card this turn."
)]
#[case::cast_only("Exile the top card of your library. You may cast that card this turn.")]
#[case::a_duration_after_the_cards(
    "Exile the top card of your library. You may play that card until end of turn."
)]
#[case::another_turn(
    "Exile the top card of your library. Until the end of your next two turns, you may play that card."
)]
#[case::without_the_period_words("Exile the top card of your library. You may play that card.")]
#[case::the_bottom("Exile the bottom card of your library. You may play that card this turn.")]
#[case::a_number_as_a_digit(
    "Exile the top 2 cards of your library. You may play those cards this turn."
)]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
