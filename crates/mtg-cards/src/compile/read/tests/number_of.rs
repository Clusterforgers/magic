//! Goal: an amount that is a count named after the sentence ("..., where X is the number of artifacts you control") or after "equal to" ("draw cards equal to the number of opponents you have") reads into the amount itself for a draw, a mill, a change of life, damage, counters, tokens, a pump and the counters a permanent enters with, writes back as the same text, and a count the compiler cannot place stays with the table.
use super::*;

fn kind_of(text: &str) -> ClauseKind {
    if text.starts_with('{') {
        ClauseKind::Activated
    } else if text.starts_with("When") || text.starts_with("At the") {
        ClauseKind::Triggered
    } else if text.starts_with("This creature enters") {
        ClauseKind::Static
    } else {
        ClauseKind::Spell
    }
}

#[rstest]
#[case::pump_power(
    "{1}{R}: This creature gets +X/+0 until end of turn, where X is the number of artifacts you control."
)]
#[case::pump_a_group_by_attackers(
    "{3}{W}{W}: Attacking creatures you control get +X/+X until end of turn, where X is the number of attacking creatures."
)]
#[case::pump_a_target(
    "{2}, {T}: Target creature gets +X/+X until end of turn, where X is the number of Gates you control. Activate only as a sorcery."
)]
#[case::pump_in_a_trigger(
    "Whenever this creature attacks, it gets +X/+X until end of turn, where X is the number of creatures you control."
)]
#[case::pump_by_total_power(
    "Target creature gets +X/+X until end of turn, where X is the total power of creatures you control."
)]
#[case::draw("{5}, {T}: Draw X cards, where X is the number of Bobbleheads you control.")]
#[case::draw_by_card_types(
    "Draw X cards, where X is the number of card types among cards in all graveyards."
)]
#[case::lose_life(
    "{2}{B}, {T}: Each opponent loses X life, where X is the number of creatures with defender you control."
)]
#[case::gain_life("You gain X life, where X is the number of Knights you control.")]
#[case::counters(
    "{3}, {T}: Put X +1/+1 counters on target creature, where X is the number of Bobbleheads you control. Activate only as a sorcery."
)]
#[case::scry(
    "At the beginning of your upkeep, scry X, where X is the number of planeswalkers you control."
)]
#[case::damage("~ deals X damage to each creature, where X is the number of lands you control.")]
#[case::tokens_in_a_zone(
    "When this creature dies, create X 1/1 white Spirit creature tokens with flying, where X is the number of creature cards in your graveyard."
)]
#[case::tokens_of_the_counters(
    "{5}, {T}, Sacrifice this artifact: Create X tapped 2/2 colorless Scarecrow artifact creature tokens, where X is the number of charge counters on this artifact."
)]
#[case::opponents("Create X Treasure tokens, where X is the number of opponents you have.")]
fn a_count_named_after_x_reads_and_writes_back(#[case] text: &str) {
    golden(text, kind_of(text), false);
}

#[rstest]
#[case::draw("When this creature enters, draw cards equal to the number of opponents you have.")]
#[case::draw_in_a_spell("Draw cards equal to the number of cards in your hand.")]
#[case::mill("Target player mills cards equal to the number of creatures you control.")]
#[case::life_lost_by_that_player(
    "Whenever this creature deals combat damage to a player, that player loses life equal to the number of creature cards in your graveyard."
)]
#[case::life_gained(
    "Whenever this creature deals combat damage to a player, you gain life equal to the number of creatures you control."
)]
#[case::damage_after_the_target(
    "~ deals damage to target creature equal to the number of lands you control."
)]
#[case::damage_before_the_target(
    "When this Vehicle enters, it deals damage equal to the number of Vehicles you control to target creature or planeswalker an opponent controls."
)]
#[case::damage_twice(
    "When this Vehicle enters, it deals damage equal to twice the number of Vehicles you control to target creature or planeswalker an opponent controls."
)]
#[case::tokens(
    "Whenever this creature attacks, create a number of 2/2 white Knight creature tokens with vigilance equal to the number of opponents you have."
)]
#[case::tokens_of_the_counters(
    "At the beginning of your end step, create a number of 1/1 white Spirit creature tokens equal to the number of +1/+1 counters on this creature."
)]
#[case::counters(
    "At the beginning of your end step, put a number of +1/+1 counters on target creature you control equal to the number of creatures you control."
)]
fn a_count_after_equal_to_reads_and_writes_back(#[case] text: &str) {
    golden(text, kind_of(text), false);
}

#[rstest]
#[case::cards_in_graveyards(
    "This creature enters with X +1/+1 counters on it, where X is the number of instant and sorcery cards in all graveyards."
)]
#[case::creatures_on_the_battlefield(
    "This creature enters with X +1/+1 counters on it, where X is the number of other creatures on the battlefield."
)]
#[case::total_mana_value(
    "This creature enters with X +1/+1 counters on it, where X is the total mana value of instant and sorcery cards in your graveyard."
)]
#[case::total_toughness(
    "This creature enters with X +1/+1 counters on it, where X is the total toughness of other creatures you control."
)]
#[case::cards_in_all_hands(
    "This creature enters with X +1/+1 counters on it, where X is the total number of cards in all players' hands."
)]
#[case::tapped(
    "This creature enters tapped with X charge counters on it, where X is the number of lands you control."
)]
fn a_permanent_enters_with_as_many_counters_as_a_count(#[case] text: &str) {
    golden(text, kind_of(text), false);
}

#[rstest]
#[case::equipped(
    "Equipped creature gets +X/+X, where X is the number of creature cards in all graveyards."
)]
#[case::enchanted("Enchanted creature gets +X/+0, where X is the number of artifacts you control.")]
#[case::the_card_itself(
    "This creature gets +X/+0, where X is the greatest power among creature cards in your graveyard."
)]
#[case::a_group(
    "Other creatures you control get +X/+X, where X is the number of creatures your opponents control."
)]
fn a_static_pump_counts_what_the_clause_names(#[case] text: &str) {
    golden(text, ClauseKind::Static, false);
}

#[rstest]
#[case::lands(
    "{1}{R}: This creature gets +X/+0 until end of turn, where X is the number of lands you control.",
    r#"{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}"#
)]
#[case::attackers(
    "Target creature gets +X/+X until end of turn, where X is the number of attacking creatures.",
    r#"{"Count":[{"Type":"Creature"},"Attacking"]}"#
)]
#[case::opponents(
    "When this creature enters, draw cards equal to the number of opponents you have.",
    r#"{"Players":"Opponents"}"#
)]
#[case::counters_on_the_card(
    "At the beginning of your end step, create a number of 1/1 white Spirit creature tokens equal to the number of +1/+1 counters on this creature.",
    r#"{"Counters":{"on":"This","counter":"+1/+1"}}"#
)]
#[case::twice(
    "When this Vehicle enters, it deals damage equal to twice the number of Vehicles you control to target creature or planeswalker an opponent controls.",
    r#"{"Sum":[{"Count":[{"Subtype":"Vehicle"},{"ControlledBy":"You"}]},{"Count":[{"Subtype":"Vehicle"},{"ControlledBy":"You"}]}]}"#
)]
#[case::total_power(
    "Target creature gets +X/+X until end of turn, where X is the total power of creatures you control.",
    r#"{"Total":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}}"#
)]
fn the_amount_is_the_count_the_clause_names(#[case] text: &str, #[case] amount: &str) {
    let json = serde_json::to_string(&golden(text, kind_of(text), false)).expect("json");
    assert!(json.contains(amount), "{text}: {json}");
    assert!(!json.contains(r#""X""#), "{text}: {json}");
}

#[rstest]
#[case::held_by_a_target("Draw cards equal to the number of cards in target opponent's hand.")]
#[case::held_by_their_hand(
    "~ deals damage to any target equal to the number of cards in their hand."
)]
#[case::the_difference("Draw cards equal to the difference.")]
#[case::the_greatest(
    "Each player discards their hand, then draws cards equal to the greatest number of cards a player exiled this way."
)]
#[case::a_count_of_what_happened(
    "Draw X cards, where X is the number of creatures that died under your control this turn."
)]
#[case::two_variables(
    "Target creature gets +X/+Y until end of turn, where X is the number of lands you control."
)]
#[case::no_x_in_the_sentence("Draw two cards, where X is the number of lands you control.")]
#[case::a_count_that_is_not_the_end(
    "Draw X cards, where X is the number of lands you control, then discard a card."
)]
#[case::the_total_of_creatures_you_control(
    "Draw X cards, where X is the total number of creatures you control."
)]
#[case::an_enters_count_of_a_characteristic(
    "This creature enters with X +1/+1 counters on it, where X is its power."
)]
#[case::an_enters_count_that_is_unread(
    "This creature enters with X +1/+1 counters on it, where X is the number of cards in target opponent's hand."
)]
fn a_count_the_compiler_cannot_place_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, kind_of(text), false), None, "{text}");
}

#[test]
fn a_count_after_equal_to_is_not_a_bare_number() {
    // Fixed numbers have their own wording ("draw two cards"); "equal to" needs a count.
    let text = "Draw cards equal to two.";
    assert_eq!(clause(text, ClauseKind::Spell, false), None);
}
