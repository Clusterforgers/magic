//! Goal: the w16 wordings (a card's pronoun, "dies", one duration, ability words and labels, articles, creature types, unordered lists, "when" and "whenever", number words, one object named twice, joined actions) read as the text they mean, and a text of another meaning never reads as the clause.
use super::*;

#[rstest]
#[case::her(
    "When this creature enters, exile it.",
    "When ~ enters, exile her.",
    true
)]
#[case::her_possessive(
    "Put its counters on target creature.",
    "Put her counters on target creature.",
    true
)]
#[case::she(
    "When this creature enters, it deals 1 damage to any target.",
    "When ~ enters, she deals 1 damage to any target.",
    true
)]
#[case::them(
    "When this creature enters, exile it.",
    "When ~ enters, exile them.",
    false
)]
#[case::their(
    "Put its counters on target creature.",
    "Put their counters on target creature.",
    false
)]
#[case::dies(
    "When this creature dies, draw a card.",
    "When ~ is put into a graveyard from the battlefield, draw a card.",
    true
)]
#[case::dies_other_zone(
    "When this creature dies, draw a card.",
    "When ~ is put into a graveyard from your hand, draw a card.",
    false
)]
fn w16_card_words(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::duration_first(
    "Target creature gets +2/+0 until end of turn.",
    "Until end of turn, target creature gets +2/+0.",
    true
)]
#[case::duration_middle(
    "Target creature gets +2/+0 until end of turn and gains trample.",
    "Until end of turn, target creature gets +2/+0 and gains trample.",
    true
)]
#[case::duration_dropped(
    "Target creature gets +2/+0 until end of turn.",
    "Target creature gets +2/+0.",
    false
)]
#[case::other_bonus(
    "Target creature gets +2/+0 until end of turn.",
    "Until end of turn, target creature gets +0/+2.",
    false
)]
#[case::two_sentences(
    "Draw a card. Target creature gets +1/+1 until end of turn.",
    "Target creature gets +1/+1. Draw a card until end of turn.",
    false
)]
#[case::ability_word(
    "Landfall \u{2014} Whenever a land you control enters, you gain 1 life.",
    "Whenever a land you control enters, you gain 1 life.",
    true
)]
#[case::ability_word_other_effect(
    "Landfall \u{2014} Whenever a land you control enters, you gain 1 life.",
    "Whenever a land you control enters, you gain 2 life.",
    false
)]
#[case::label(
    "Survival \u{2014} At the beginning of your second main phase, draw a card.",
    "At the beginning of your second main phase, draw a card.",
    true
)]
#[case::label_of_a_cost("Exhaust \u{2014} {4}: Draw a card.", "{4}: Draw a card.", false)]
#[case::chapter("III \u{2014} Draw a card.", "Draw a card.", false)]
fn w16_durations_and_labels(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::an_untapped(
    "Tap an untapped creature you control.",
    "Tap a untapped creature you control.",
    true
)]
#[case::untapped_and_tapped(
    "Tap an untapped creature you control.",
    "Tap a tapped creature you control.",
    false
)]
#[case::singular_type(
    "Put a +1/+1 counter on each Ally creature you control.",
    "Put a +1/+1 counter on each Ally you control.",
    true
)]
#[case::singular_type_all_creatures(
    "Put a +1/+1 counter on each Ally creature you control.",
    "Put a +1/+1 counter on each creature you control.",
    false
)]
#[case::plural_type("All Goblins get +1/+1.", "All Goblin creatures get +1/+1.", true)]
#[case::other_plural_type("All Goblins get +1/+1.", "All Elf creatures get +1/+1.", false)]
#[case::type_cards(
    "Return all Doctor cards from your graveyard to your hand.",
    "Return all Doctor creature cards from your graveyard to your hand.",
    false
)]
#[case::the_creatures(
    "~ deals 2 damage to the creature's controller.",
    "~ deals 2 damage to that creature's controller.",
    true
)]
#[case::the_creatures_owner(
    "~ deals 2 damage to the creature's controller.",
    "~ deals 2 damage to that creature's owner.",
    false
)]
fn w16_articles_and_types(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::card_types_in_any_order(
    "Whenever another creature or Vehicle you control enters, draw a card.",
    "Whenever another Vehicle or creature you control enters, draw a card.",
    true
)]
#[case::keywords(
    "Target creature gains hexproof and haste until end of turn.",
    "Target creature gains haste and hexproof until end of turn.",
    true
)]
#[case::three_items(
    "Destroy target artifact, creature, or land.",
    "Destroy target land, artifact, or creature.",
    true
)]
#[case::colors(
    "Create a 1/1 black and white Spirit creature token.",
    "Create a 1/1 white and black Spirit creature token.",
    true
)]
#[case::trigger_events(
    "Whenever this creature enters or dies, draw a card.",
    "When ~ dies or enters, draw a card.",
    true
)]
#[case::and_for_or(
    "Destroy target artifact or enchantment.",
    "Destroy target artifact and enchantment.",
    false
)]
#[case::other_item(
    "Destroy target artifact or enchantment.",
    "Destroy target artifact or land.",
    false
)]
#[case::each_and_is_or(
    "Each land and Ally you control has \"{T}: Add one mana of any color.\"",
    "Each land or Ally you control has \"{T}: Add one mana of any color.\"",
    true
)]
#[case::target_and_is_not_or("Untap target land and Ally.", "Untap target land or Ally.", false)]
#[case::actions_keep_their_order(
    "Discard a card or pay 3 life.",
    "Pay 3 life or discard a card.",
    false
)]
fn w16_unordered_lists(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::when_whenever(
    "When a creature you control dies, draw a card.",
    "Whenever a creature you control dies, draw a card.",
    true
)]
#[case::quoted_trigger(
    "Creatures you control have \"Whenever this creature attacks, draw a card.\"",
    "Creatures you control have \"When ~ attacks, draw a card.\"",
    true
)]
#[case::number_words("Mill four cards.", "Mill 4 cards.", true)]
#[case::other_number("Mill four cards.", "Mill 5 cards.", false)]
#[case::greater(
    "If you have 7 or greater life, draw a card.",
    "If you have 7 or more life, draw a card.",
    true
)]
#[case::fewer(
    "If you have 7 or fewer life, draw a card.",
    "If you have 7 or more life, draw a card.",
    false
)]
#[case::other_target(
    "Destroy up to two other target creatures.",
    "Destroy up to two target other creatures.",
    true
)]
#[case::also("It also gains haste.", "It gains haste.", true)]
#[case::team(
    "Knights your team controls have flying.",
    "Knights you control have flying.",
    true
)]
#[case::opponents(
    "Knights your team controls have flying.",
    "Knights your opponents control have flying.",
    false
)]
#[case::was_is(
    "Counter target spell. If its mana value was 3 or less, proliferate.",
    "Counter target spell. If its mana value is 3 or less, proliferate.",
    true
)]
#[case::was_is_other_value(
    "Counter target spell. If its mana value was 3 or less, proliferate.",
    "Counter target spell. If its mana value is 4 or less, proliferate.",
    false
)]
fn w16_forms(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::that_creature(
    "Tap target creature. Untap that creature.",
    "Tap target creature. Untap it.",
    true
)]
#[case::that_card(
    "Exile target creature. Return that card to the battlefield.",
    "Exile target creature. Return that creature to the battlefield.",
    true
)]
#[case::two_kinds(
    "Whenever a creature or land becomes tapped, put a counter on it.",
    "Whenever a creature or land becomes tapped, put a counter on that creature.",
    false
)]
#[case::attached(
    "Create a 2/2 white Cat creature token for each Equipment attached to ~.",
    "Create a 2/2 white Cat creature token for each Equipment attached to it.",
    true
)]
#[case::attached_to_this_creature(
    "This creature gets +2/+2 for each Aura and Equipment attached to it.",
    "~ gets +2/+2 for each Equipment and/or Aura attached to her.",
    true
)]
#[case::attached_to_a_target(
    "Create a 2/2 white Cat creature token for each Equipment attached to target creature.",
    "Create a 2/2 white Cat creature token for each Equipment attached to it.",
    false
)]
#[case::attached_aura(
    "Create a 2/2 white Cat creature token for each Equipment attached to ~.",
    "Create a 2/2 white Cat creature token for each Aura attached to it.",
    false
)]
fn w16_one_object(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}

#[rstest]
#[case::lines_are_one_text(
    "Support 2.\nDraw a card.",
    "Put a +1/+1 counter on each of up to two other target creatures, then draw a card.",
    true
)]
#[case::then_draw(
    "Each player loses 2 life. You draw two cards.",
    "Each player loses 2 life. Then draw two cards.",
    true
)]
#[case::another_action(
    "Each player loses 2 life. You draw two cards.",
    "Each player loses 2 life. You discard two cards.",
    false
)]
#[case::another_player(
    "Each player loses 2 life. You draw two cards.",
    "Each player loses 2 life. Each player draws two cards.",
    false
)]
#[case::you_in_a_trigger(
    "When this creature dies, you gain control of all enchantments.",
    "When ~ dies, gain control of all enchantments.",
    true
)]
#[case::and_an_order(
    "Exile target creature and draw a card.",
    "Exile target creature. Draw a card.",
    true
)]
#[case::and_a_player(
    "At the beginning of your upkeep, sacrifice ~ and target opponent discards their hand.",
    "At the beginning of your upkeep, sacrifice ~. Target opponent discards their hand.",
    true
)]
#[case::comma_list(
    "Search your library for a card, put it into your hand, shuffle.",
    "Search your library for a card, put it into your hand. Shuffle.",
    true
)]
#[case::and_with_may(
    "You may exile target creature and draw a card.",
    "You may exile target creature. Draw a card.",
    false
)]
#[case::and_after_a_condition(
    "If you control a Goblin, exile target creature and draw a card.",
    "If you control a Goblin, exile target creature. Draw a card.",
    false
)]
#[case::and_after_if_you_do(
    "Sacrifice a creature. If you do, exile target creature and draw a card.",
    "Sacrifice a creature. If you do, exile target creature. Draw a card.",
    false
)]
#[case::other_second_action(
    "Exile target creature and draw a card.",
    "Exile target creature. Discard a card.",
    false
)]
fn w16_joined_actions(#[case] left: &str, #[case] right: &str, #[case] same: bool) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}
