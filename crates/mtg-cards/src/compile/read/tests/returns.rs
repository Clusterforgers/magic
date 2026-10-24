//! Goal: "return target card from your graveyard to your hand" reads into a move to the hand with the card's zone and owner in the target, alone, with a card type, in a trigger (mandatory when the text has no "may") and after "you may", and "return a land you control to its owner's hand" reads as a choice and a move; any other place, owner, count or destination stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::a_card(
    "Return target card from your graveyard to your hand.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::a_creature_card(
    "Return target creature card from your graveyard to your hand.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::an_instant_or_sorcery_card(
    "Return target instant or sorcery card from your graveyard to your hand.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
fn a_return_reads_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn eternal_witness_reads_in_the_shape_of_its_file() {
    reads_as(
        "When ~ enters, you may return target card from your graveyard to your hand.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#,
    );
}

#[test]
fn timeless_witness_returns_the_card_without_a_choice() {
    // The Oracle text has no "may": the trigger is mandatory (CR 603.5 makes only "may" effects optional).
    reads_as(
        "When this creature enters, return target card from your graveyard to your hand.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#,
    );
}

#[rstest]
#[case("Return target artifact card from your graveyard to your hand.")]
#[case("Return target enchantment card from your graveyard to your hand.")]
#[case("Return target land card from your graveyard to your hand.")]
#[case("Return target planeswalker card from your graveyard to your hand.")]
#[case("Return target creature or planeswalker card from your graveyard to your hand.")]
#[case("Return target nonland card from your graveyard to your hand.")]
#[case("Return target Elf card from your graveyard to your hand.")]
#[case("Return target noncreature card from your graveyard to your hand.")]
#[case("Return target legendary creature card from your graveyard to your hand.")]
#[case("Return target card from your graveyard to your hand. Draw a card.")]
#[case("Draw a card. Return target creature card from your graveyard to your hand.")]
#[case("Return target creature card from your graveyard to your hand. You gain 2 life.")]
#[case("You may return target creature card from your graveyard to your hand.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ dies, return target creature card from your graveyard to your hand.")]
#[case("When ~ enters, return target card from your graveyard to your hand.")]
#[case("When ~ enters, you may return target creature card from your graveyard to your hand.")]
#[case("Whenever ~ attacks, you may return target artifact card from your graveyard to your hand.")]
#[case("At the beginning of your upkeep, return target card from your graveyard to your hand.")]
#[case(
    "Whenever a creature you control dies, you may return target card from your graveyard to your hand."
)]
fn more_oracle_wordings_in_a_trigger_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::bounce_to_your_hand("Return target creature you control to your hand.")]
#[case::another_graveyard("Return target card from a graveyard to your hand.")]
#[case::an_opponents_graveyard(
    "Return target creature card from an opponent's graveyard to your hand."
)]
#[case::your_library("Return target card from your library to your hand.")]
#[case::the_top_of_the_library(
    "Return target card from your graveyard to the top of your library."
)]
#[case::all("Return all creature cards from your graveyard to your hand.")]
#[case::each("Return each creature card from your graveyard to your hand.")]
#[case::this_card("Return this card from your graveyard to your hand.")]
#[case::a_chain("Return target card from your graveyard to your hand, then discard a card.")]
#[case::without_a_zone("Return target card to your hand.")]
#[case::a_creature_without_card("Return target creature from your graveyard to your hand.")]
#[case::a_wrong_agreement("Return target cards from your graveyard to your hand.")]
#[case::to_its_owners_hand("Return target card from your graveyard to its owner's hand.")]
#[case::a_second_target(
    "Return target card from your graveyard to your hand. Exile target creature."
)]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn a_card_in_a_graveyard_is_not_a_phrase_without_its_zone() {
    assert_eq!(spell("Exile target card."), None);
    assert_eq!(spell("Destroy target creature card."), None);
    assert_eq!(trigger("When ~ enters, exile target card."), None);
}

#[rstest]
#[case::a_land(
    "When this land enters, return a land you control to its owner's hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}]}}"#
)]
#[case::a_creature(
    "When ~ enters, return a creature you control to its owner's hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}]}}"#
)]
fn a_permanent_you_choose_returns_to_its_owners_hand(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::any_land("When this land enters, return a land to its owner's hand.")]
#[case::an_opponents("When ~ enters, return a creature an opponent controls to its owner's hand.")]
#[case::from_a_graveyard(
    "When ~ enters, return a creature card from your graveyard to its owner's hand."
)]
#[case::to_your_hand("When ~ enters, return a land you control to your hand.")]
#[case::two("When ~ enters, return two lands you control to their owner's hand.")]
#[case::another("When ~ enters, return another creature you control to its owner's hand.")]
fn a_choice_of_the_return_stays_with_the_table_unless_the_player_controls_it(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}
