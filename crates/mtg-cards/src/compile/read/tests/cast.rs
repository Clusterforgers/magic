//! Goal: "Discover 4.", the exile from the top of a library until a card is found with the free cast of it and the move of the cards exiled to the bottom in a random order, read as the three effects they stand for, alone and in a trigger, written back as the same text, and left to the table when the cast has nothing it follows.
use super::*;
use crate::effect::{Effect, discover};
use crate::refs::Ref;
use crate::value::Amount;

#[rstest]
#[case::discover("Discover 4.", 4)]
#[case::discover_one("Discover 1.", 1)]
fn discover_reads_as_its_three_effects(#[case] text: &str, #[case] count: i32) {
    let expected = Ability::Spell {
        targets: Vec::new(),
        effects: discover(Amount::Fixed(count)),
    };
    assert_eq!(golden(text, ClauseKind::Spell, false), expected);
}

#[test]
fn discover_in_a_trigger_reads_and_writes_back() {
    let read = golden("When ~ enters, discover 5.", ClauseKind::Triggered, false);
    let Ability::Triggered { effects, .. } = read else {
        panic!("a triggered ability");
    };
    assert_eq!(effects, discover(Amount::Fixed(5)));
}

#[rstest]
#[case::the_exiling(
    "Exile cards from the top of your library until you exile a nonland card with mana value 3 or less."
)]
#[case::and_the_cast(
    "Exile cards from the top of your library until you exile a nonland card with mana value 3 or less. You may cast it without paying its mana cost."
)]
fn the_first_sentences_read(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[test]
fn the_sentences_of_an_exile_until_a_card_is_found_and_its_free_cast_read_one_by_one() {
    let text = "Exile cards from the top of your library until you exile a nonland card with mana value 3 or less. You may cast it without paying its mana cost. Put all cards exiled this way that weren't cast on the bottom of your library in a random order.";
    let read = golden(text, ClauseKind::Spell, false);
    let Ability::Spell { effects, .. } = read else {
        panic!("a spell");
    };
    let expected = discover(Amount::Fixed(3));
    assert_eq!(effects[0], expected[0]);
    assert_eq!(effects[2], expected[2]);
    assert_eq!(
        effects[1],
        Effect::CastFromZone {
            who: Ref::You,
            what: Ref::Bound("found".into()),
            free: true,
            play: false,
            way: None,
            transformed: false,
            bind: None,
            otherwise: Vec::new()
        }
    );
}

#[rstest]
#[case::a_cast_with_nothing_exiled_before("You may cast it without paying its mana cost.")]
#[case::bottom_with_nothing_exiled_before(
    "Put all cards exiled this way that weren't cast on the bottom of your library in a random order."
)]
#[case::discover_without_a_number("Discover.")]
#[case::another_value(
    "Exile cards from the top of your library until you exile a nonland card with mana value 3 or more."
)]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
