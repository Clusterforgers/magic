//! Goal: cards that change zones by a choice read and write back as the oracle words them: cards of a graveyard returned to the hand or the battlefield (any number, a type, a subtype, a mana value, tapped), taken from another graveyard under the player's control, a graveyard exiled, and a card put from the hand onto the battlefield; every other owner, destination or count stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::a_permanent_card("Return target permanent card from your graveyard to your hand.")]
#[case::a_nonland_permanent_card(
    "Return target nonland permanent card from your graveyard to your hand."
)]
#[case::with_a_mana_value(
    "Return target permanent card with mana value 3 or less from your graveyard to the battlefield."
)]
#[case::a_subtype_card("Return target Elf card from your graveyard to your hand.")]
#[case::two_subtypes_and_a_type(
    "Return target Vampire or Wizard creature card from your graveyard to the battlefield."
)]
#[case::a_subtype_and_a_type(
    "Return target Zombie creature card from your graveyard to your hand."
)]
#[case::up_to_two("Return up to two target creature cards from your graveyard to your hand.")]
#[case::up_to_three_tapped(
    "Return up to three target land cards from your graveyard to the battlefield tapped."
)]
#[case::tapped("Return target creature card from your graveyard to the battlefield tapped.")]
#[case::an_artifact_or_creature(
    "Return target artifact or creature card from your graveyard to the battlefield."
)]
#[case::from_any_graveyard(
    "Put target creature card from a graveyard onto the battlefield under your control."
)]
#[case::from_an_opponents_graveyard(
    "Put target creature card from an opponent's graveyard onto the battlefield tapped under your control."
)]
#[case::a_creature_or_planeswalker(
    "Put target creature or planeswalker card from a graveyard onto the battlefield under your control."
)]
#[case::all_tapped("Return all land cards from your graveyard to the battlefield tapped.")]
#[case::exile_a_card("Exile target card from a graveyard.")]
#[case::exile_a_creature_card("Exile target creature card from a graveyard.")]
#[case::exile_a_players_graveyard("Exile target player's graveyard.")]
#[case::exile_an_opponents_graveyard("Exile target opponent's graveyard.")]
#[case::exile_all_graveyards("Exile all graveyards.")]
#[case::exile_your_graveyard("Exile your graveyard.")]
#[case::exile_each_opponents_graveyard("Exile each opponent's graveyard.")]
fn spells_that_move_cards_between_zones_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case::may_put_a_creature(
    "When ~ enters, you may put a creature card from your hand onto the battlefield."
)]
#[case::may_put_a_land(
    "When ~ enters, you may put a land card from your hand onto the battlefield tapped."
)]
#[case::may_put_a_basic_land(
    "When ~ enters, you may put a basic land card from your hand onto the battlefield tapped."
)]
#[case::from_the_defenders_graveyard(
    "Whenever ~ attacks, put target creature card from defending player's graveyard onto the battlefield under your control."
)]
#[case::at_the_upkeep(
    "At the beginning of your upkeep, put target creature card from a graveyard onto the battlefield under your control."
)]
#[case::returning_a_permanent(
    "Whenever ~ enters or attacks, you may return target permanent card with mana value 3 or less from your graveyard to the battlefield."
)]
#[case::exiling_a_graveyard("When ~ enters, exile target player's graveyard.")]
fn triggers_that_move_cards_between_zones_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::put_from_a_graveyard(
    "{T}: Put target creature card from a graveyard onto the battlefield under your control."
)]
#[case::put_from_the_hand(
    "{2}, {T}: You may put a basic land card from your hand onto the battlefield tapped."
)]
#[case::exile_a_graveyard("{T}, Sacrifice ~: Exile target player's graveyard.")]
#[case::return_all(
    "{3}{G}, Sacrifice ~: Return all land cards from your graveyard to the battlefield tapped."
)]
fn abilities_that_move_cards_between_zones_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::a_nonland_permanent(
    "Return target nonland permanent card from your graveyard to your hand.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Planeswalker"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::with_a_mana_value(
    "Return target permanent card with mana value 3 or less from your graveyard to the battlefield.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"Compare":{"stat":"ManaValue","op":"Le","to":3}},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#
)]
#[case::up_to_two(
    "Return up to two target Elf cards from your graveyard to the battlefield tapped.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Subtype":"Elf"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":2}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true}}}}]}}"#
)]
#[case::from_any_graveyard(
    "Put target creature card from a graveyard onto the battlefield under your control.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"under":"You"}}}}]}}"#
)]
#[case::from_an_opponents_graveyard(
    "Put target creature card from an opponent's graveyard onto the battlefield under your control.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"under":"You"}}}}]}}"#
)]
#[case::exile_a_graveyard(
    "Exile target player's graveyard.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}}]}}"#
)]
fn the_cards_moved_are_read_in_the_shape_of_the_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn a_card_chosen_from_the_hand_is_bound_and_put_onto_the_battlefield() {
    reads_as(
        "When ~ enters, you may put a land card from your hand onto the battlefield tapped.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Land"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{"tapped":true}}}}]}}]}}"#,
    );
}

#[rstest]
#[case::another_graveyard_to_your_hand(
    "Return target creature card from a graveyard to your hand."
)]
#[case::an_opponents_graveyard_to_the_battlefield(
    "Return target creature card from an opponent's graveyard to the battlefield."
)]
#[case::your_graveyard_under_your_control(
    "Put target creature card from your graveyard onto the battlefield under your control."
)]
#[case::another_graveyard_without_control(
    "Put target creature card from a graveyard onto the battlefield."
)]
#[case::to_its_owners_hand("Return target creature card from a graveyard to its owner's hand.")]
#[case::a_single_graveyard("Exile up to two target cards from a single graveyard.")]
#[case::a_hand("Exile target player's hand.")]
#[case::a_creatures_graveyard("Exile target creature's graveyard.")]
#[case::a_player_of_two("Exile target player's graveyard and target opponent's graveyard.")]
#[case::all_graveyards_to_the_battlefield(
    "Return all creature cards from all graveyards to the battlefield."
)]
#[case::from_the_library("Put a land card from your library onto the battlefield.")]
#[case::without_a_zone("Put a land card onto the battlefield.")]
#[case::not_a_choice("Put all land cards from your hand onto the battlefield.")]
fn other_owners_destinations_and_counts_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::onto_the_battlefield_without_may(
    "When ~ enters, put a land card from your hand onto the battlefield."
)]
#[case::attacking(
    "When ~ enters, you may put a creature card from your hand onto the battlefield tapped and attacking."
)]
#[case::from_a_graveyard(
    "When ~ enters, you may put a creature card from your graveyard onto the battlefield."
)]
#[case::under_an_opponents_control(
    "When ~ enters, you may put a land card from your hand onto the battlefield under an opponent's control."
)]
#[case::two_cards("When ~ enters, you may put two land cards from your hand onto the battlefield.")]
fn a_put_from_the_hand_is_a_may_of_one_card_to_the_battlefield(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::tokens(
    "Return up to three target land cards from your graveyard to the battlefield tapped. Create that many 4/2 green Plant Warrior creature tokens with reach."
)]
#[case::alone("Create that many Treasure tokens.")]
#[case::counters("Put that many +1/+1 counters on target creature.")]
fn that_many_is_the_amount_of_a_trigger_and_not_of_a_spell(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[test]
fn an_activated_put_from_the_hand_needs_its_may() {
    assert_eq!(
        activated("{3}, {T}: Put a land card from your hand onto the battlefield tapped."),
        None
    );
}
