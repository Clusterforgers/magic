//! Goal: the Room wordings ("when you unlock this door", Eerie's "whenever you fully unlock a Room" beside an enchantment entering, and "unlock a locked door of a Room you control") read into the DSL and write back as the card's own text.
use super::*;

#[rstest]
#[case::unlock_this_door(
    "When you unlock this door, draw a card.",
    r#"{"Triggered":{"when":{"DoorUnlocked":{"who":"You","door":0}},"effects":[{"Draw":{}}]}}"#
)]
#[case::fully_unlock(
    "Whenever you fully unlock a Room, draw a card.",
    r#"{"Triggered":{"when":{"DoorUnlocked":{"who":"You","fully":true}},"effects":[{"Draw":{}}]}}"#
)]
#[case::eerie(
    "Whenever an enchantment you control enters and whenever you fully unlock a Room, draw a card.",
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":[{"Type":"Enchantment"},{"ControlledBy":"You"}],"to":"Battlefield"}},{"DoorUnlocked":{"who":"You","fully":true}}]},"effects":[{"Draw":{}}]}}"#
)]
#[case::unlock_a_door(
    "When ~ enters, unlock a locked door of a Room you control.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"UnlockDoor":{}}]}}"#
)]
fn the_room_wordings_read(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::return_or_unlock(
    "When ~ enters, return an enchantment card from your graveyard to your hand or unlock a locked door of a Room you control.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Either":{"first":[{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Enchantment"}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}],"second":[{"UnlockDoor":{}}]}}]}}"#
)]
#[case::two_plain_sentences(
    "When ~ enters, draw a card or unlock a locked door of a Room you control.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Either":{"first":[{"Draw":{}}],"second":[{"UnlockDoor":{}}]}}]}}"#
)]
#[case::a_card_returned_alone(
    "When ~ enters, return a creature card from your graveyard to your hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}]}}"#
)]
fn a_choice_between_two_sentences_reads(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::an_option_that_does_not_read(
    "When ~ enters, draw a card or frobnicate a Room you control."
)]
#[case::no_second_option("When ~ enters, draw a card or.")]
#[case::an_option_that_is_a_may("When ~ enters, you may draw a card or discard a card.")]
#[case::another_graveyard(
    "When ~ enters, return a creature card from an opponent's graveyard to your hand."
)]
#[case::the_owners_hand(
    "When ~ enters, return a creature card from your graveyard to its owner's hand."
)]
fn a_choice_the_table_words_differently_stays_unread(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
