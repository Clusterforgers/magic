//! Goal: an amount that is the power, toughness or mana value of an object is worded X and named at the end of the sentence ("..., where X is its power"); it reads into the amount itself and writes back as the same text, and a clause whose object, characteristic or X is unclear stays with the table.
use super::*;

fn kind_of(text: &str) -> ClauseKind {
    if text.starts_with('{') {
        ClauseKind::Activated
    } else if text.starts_with("When") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    }
}

#[rstest]
#[case::pump_activated("{2}{G}: This creature gets +X/+X until end of turn, where X is its power.")]
#[case::pump_named("{2}{G}: ~ gets +X/+X until end of turn, where X is its toughness.")]
#[case::counters_activated("{2}{G}: Put X +1/+1 counters on this creature, where X is its power.")]
#[case::life_activated(
    "{2}{B}, Sacrifice this creature: Target player loses X life, where X is this creature's power."
)]
#[case::pump_target("Target creature gets +X/+X until end of turn, where X is its toughness.")]
#[case::pump_target_power_only(
    "Target creature gets +X/+0 until end of turn, where X is its power."
)]
#[case::tokens_when_it_dies(
    "When this creature dies, create X 1/1 white Vampire creature tokens with lifelink, where X is its power."
)]
#[case::tokens_when_the_card_dies(
    "When ~ dies, create X tapped 1/1 green Squirrel creature tokens, where X is ~'s power."
)]
#[case::life_when_it_leaves(
    "When this creature leaves the battlefield, each opponent loses X life and you gain X life, where X is its power."
)]
#[case::damage_and_life(
    "When ~ leaves the battlefield, it deals X damage to target player and you gain X life, where X is its power."
)]
#[case::pump_the_others(
    "Whenever ~ attacks, other attacking creatures get +X/+0 until end of turn, where X is ~'s power."
)]
#[case::pump_the_others_you_control(
    "Whenever this creature attacks, other creatures you control get +X/+X until end of turn, where X is this creature's power."
)]
#[case::counters_on_the_card_for_another(
    "Whenever another creature enters, you may put X +1/+1 counters on this creature, where X is that creature's power."
)]
#[case::counters_on_the_card_when_another_dies(
    "Whenever another creature dies, you may put X +1/+1 counters on ~, where X is that creature's power."
)]
#[case::the_dying_creature_of_a_group(
    "Whenever a creature you control dies, draw X cards, where X is its power."
)]
#[case::mana_value("When ~ dies, you gain X life, where X is its mana value.")]
#[case::a_number_of_tokens(
    "Whenever this creature attacks, create a number of 1/1 white Rabbit creature tokens equal to this creature's power."
)]
#[case::a_number_of_tokens_for_another(
    "Whenever another creature enters, create a number of 1/1 white Spirit creature tokens equal to that creature's toughness."
)]
fn a_characteristic_named_after_x_reads_and_writes_back(#[case] text: &str) {
    golden(text, kind_of(text), false);
}

#[rstest]
#[case::the_card_in_an_activation(
    "{2}{G}: This creature gets +X/+X until end of turn, where X is its power.",
    r#""Stat":{"of":"This","stat":"Power"}"#
)]
#[case::the_target(
    "Target creature gets +X/+X until end of turn, where X is its toughness.",
    r#""Stat":{"of":{"Target":0},"stat":"Toughness"}"#
)]
#[case::the_creature_of_the_event(
    "Whenever another creature enters, you may put X +1/+1 counters on this creature, where X is that creature's power.",
    r#""Stat":{"of":"Triggering","stat":"Power"}"#
)]
#[case::the_card_that_died(
    "When this creature dies, each opponent loses X life and you gain X life, where X is its power.",
    r#""Stat":{"of":"This","stat":"Power"}"#
)]
fn the_amount_is_the_characteristic_of_the_object_the_clause_names(
    #[case] text: &str,
    #[case] amount: &str,
) {
    let json = serde_json::to_string(&golden(text, kind_of(text), false)).expect("json");
    assert!(json.contains(amount), "{text}: {json}");
    assert!(!json.contains(r#""X""#), "{text}: {json}");
}

#[rstest]
#[case::unknown_characteristic(
    "Target creature gets +X/+X until end of turn, where X is its colors."
)]
#[case::no_x_in_the_sentence("Target creature gets +2/+2 until end of turn, where X is its power.")]
#[case::no_object_for_its(
    "Creatures you control get +X/+X until end of turn, where X is its power."
)]
#[case::its_in_a_spell_trigger("Whenever you cast a spell, you gain X life, where X is its power.")]
#[case::a_count_held_by_a_player(
    "Target creature gets +X/+X until end of turn, where X is the number of cards in target player's hand."
)]
#[case::another_object(
    "Target creature gets +X/+X until end of turn, where X is target creature's power."
)]
#[case::a_second_variable("Target creature gets +X/+Y until end of turn, where X is its power.")]
#[case::x_in_another_sentence(
    "Target creature gets +X/+X until end of turn. Draw a card, where X is its power."
)]
#[case::a_number_of_without_a_count(
    "Whenever this creature attacks, create a number of 1/1 white Rabbit creature tokens."
)]
#[case::a_number_of_equal_to_a_life_total(
    "Whenever this creature attacks, create a number of 1/1 white Rabbit creature tokens equal to your life total."
)]
#[case::a_number_of_for_an_unclear_object(
    "Whenever you cast a spell, create a number of 1/1 white Rabbit creature tokens equal to its power."
)]
#[case::a_stat_of_a_player(
    "Target creature gets +X/+X until end of turn, where X is your life total."
)]
fn near_misses_of_a_where_clause_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, kind_of(text), false), None, "{text}");
}

#[test]
fn the_text_without_a_where_clause_still_needs_the_announced_x() {
    let text = "Target creature gets +X/+X until end of turn.";
    assert_eq!(clause(text, ClauseKind::Spell, false), None);
    assert!(clause(text, ClauseKind::Spell, true).is_some());
}

#[test]
fn a_where_clause_gives_the_x_of_the_cost_its_amount() {
    let text = "Target creature gets +X/+X until end of turn, where X is its power.";
    let read = golden(text, ClauseKind::Spell, true);
    let json = serde_json::to_string(&read).expect("json");
    assert!(json.contains("Stat"), "{json}");
}
