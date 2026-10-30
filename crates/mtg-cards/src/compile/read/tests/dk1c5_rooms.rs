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
