//! Goal: "unless ... pays {M}" reads as a `Pay` whose unpaid effects are the sentence before it, for the controller of a countered spell and for the player a spell-cast trigger is about; any other payer, cost or sentence stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::mana_leak(
    "Counter target spell unless its controller pays {3}.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":"{3}","unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::spell_pierce(
    "Counter target noncreature spell unless its controller pays {2}.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Not":{"Type":"Creature"}}]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":"{2}","unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::colored_mana(
    "Counter target spell unless its controller pays {1}{U}.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":"{1}{U}","unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::then_a_sentence(
    "Counter target spell unless its controller pays {1}. You gain 2 life.",
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":"{1}","unpaid":[{"Counter":{"what":{"Target":0}}}]}},{"GainLife":{"amount":2}}]}}"#
)]
fn a_payment_in_a_spell_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::rhystic_study(
    "Whenever an opponent casts a spell, you may draw a card unless that player pays {1}.",
    r#"{"Triggered":{"when":{"SpellCast":{"by":"Opponents"}},"effects":[{"Pay":{"who":"Triggering","cost":"{1}","unpaid":[{"May":{"effects":[{"Draw":{}}]}}]}}]}}"#
)]
#[case::mystic_remora(
    "Whenever an opponent casts a noncreature spell, you may draw a card unless that player pays {4}.",
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"Opponents"}},"effects":[{"Pay":{"who":"Triggering","cost":"{4}","unpaid":[{"May":{"effects":[{"Draw":{}}]}}]}}]}}"#
)]
#[case::without_a_choice(
    "Whenever an opponent casts a spell, you gain 1 life unless that player pays {2}.",
    r#"{"Triggered":{"when":{"SpellCast":{"by":"Opponents"}},"effects":[{"Pay":{"who":"Triggering","cost":"{2}","unpaid":[{"GainLife":{"amount":1}}]}}]}}"#
)]
#[case::any_player_casts(
    "Whenever a player casts a spell, draw a card unless that player pays {1}.",
    r#"{"Triggered":{"when":{"SpellCast":{}},"effects":[{"Pay":{"who":"Triggering","cost":"{1}","unpaid":[{"Draw":{}}]}}]}}"#
)]
fn a_payment_in_a_trigger_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Counter target spell unless its controller pays {1}.")]
#[case("Counter target spell unless its controller pays {2}.")]
#[case("Counter target spell unless its controller pays {4}.")]
#[case("Counter target spell unless its controller pays {0}.")]
#[case("Counter target spell unless its controller pays {R}.")]
#[case("Counter target spell unless its controller pays {2}{G}.")]
#[case("Counter target creature spell unless its controller pays {2}.")]
#[case("Counter target instant or sorcery spell unless its controller pays {3}.")]
#[case("Counter target spell unless its controller pays {W/U}.")]
fn more_oracle_wordings_of_a_countered_spell_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("Whenever an opponent casts a spell, you may draw a card unless that player pays {2}.")]
#[case(
    "Whenever an opponent casts a creature spell, you may draw a card unless that player pays {3}."
)]
#[case("Whenever you cast a spell, you may draw a card unless that player pays {1}.")]
#[case("Whenever a player casts a spell, you may scry 1 unless that player pays {1}.")]
#[case("Whenever an opponent casts a spell, you gain 2 life unless that player pays {1}.")]
fn more_oracle_wordings_of_a_spell_cast_trigger_read_and_write_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::not_a_counter("Destroy target creature unless its controller pays {3}.")]
#[case::exile("Exile target creature unless its controller pays {3}.")]
#[case::you_pay("Counter target spell unless you pay {1}.")]
#[case::that_player_in_a_spell("Counter target spell unless that player pays {1}.")]
#[case::they("Counter target spell unless they pay {1}.")]
#[case::x("Counter target spell unless its controller pays {X}.")]
#[case::no_cost("Counter target spell unless its controller pays.")]
#[case::life("Counter target spell unless its controller pays 3 life.")]
#[case::tap("Counter target spell unless its controller pays {T}.")]
#[case::a_number("Counter target spell unless its controller pays 3.")]
#[case::for_each("Counter target spell unless its controller pays {1} for each card in your hand.")]
#[case::where_x("Counter target spell unless its controller pays {X}, where X is its mana value.")]
#[case::an_additional_cost(
    "Counter target spell unless its controller pays {2} and discards a card."
)]
#[case::an_activated_ability("Counter target activated ability unless its controller pays {2}.")]
#[case::no_object("Draw a card unless its controller pays {1}.")]
#[case::unless_first("Unless its controller pays {2}, counter target spell.")]
#[case::unless_alone("Unless its controller pays {2}.")]
#[case::unless_after_a_sentence("Counter target spell unless its controller pays {2}. Unless")]
fn a_payment_the_template_does_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::no_event_player("When ~ enters, you may draw a card unless that player pays {1}.")]
#[case::a_creature_event(
    "Whenever a creature you control dies, draw a card unless that player pays {1}."
)]
#[case::step("At the beginning of your upkeep, you may draw a card unless that player pays {1}.")]
#[case::sacrifice_event(
    "Whenever you sacrifice a creature, draw a card unless that player pays {1}."
)]
#[case::you_pay("Whenever an opponent casts a spell, you may draw a card unless you pay {1}.")]
#[case::they_pay("Whenever an opponent casts a spell, you may draw a card unless they pay {1}.")]
#[case::x("Whenever an opponent casts a spell, you may draw a card unless that player pays {X}.")]
#[case::x_equal_to_power(
    "Whenever an opponent casts a spell, you may draw a card unless that player pays {X}, where X is ~'s power."
)]
#[case::a_chain(
    "Whenever an opponent casts a spell, you may draw a card, then discard a card unless that player pays {1}."
)]
#[case::unless_first(
    "Whenever an opponent casts a spell, unless that player pays {1}, you gain 1 life."
)]
#[case::a_second_payer(
    "Whenever an opponent casts a spell, you may draw a card unless that player pays {1} unless that player pays {1}."
)]
fn a_payment_in_a_trigger_the_template_does_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[test]
fn a_spell_cast_trigger_is_the_only_event_that_names_a_player() {
    // The same sentence reads after the spell-cast event and nowhere else.
    let tail = "you may draw a card unless that player pays {1}.";
    assert!(trigger(&format!("Whenever an opponent casts a spell, {tail}")).is_some());
    assert!(trigger(&format!("Whenever a creature you control dies, {tail}")).is_none());
    assert!(spell(tail).is_none());
}
