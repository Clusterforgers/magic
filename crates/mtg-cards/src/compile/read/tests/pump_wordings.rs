//! Goal: a pump that gives protection from the color of your choice, a pump or a counter on "it" in the trigger of a creature (the card itself or the creature of the event), the batch trigger "one or more +1/+1 counters are put on", and a noun "with a +1/+1 counter on it" read into the DSL and write back as the same text; the near-misses stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn static_ability(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::activated_tap(
    "{T}: Target creature you control gains protection from the color of your choice until end of turn.",
    r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["AddProtectionFromChosenColor"],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::with_a_stat_bonus(
    "Target creature gets +2/+2 and gains protection from the color of your choice until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":2}},"AddProtectionFromChosenColor"],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::after_the_card_untapped(
    "Whenever you cast a noncreature spell, untap ~. It gains protection from the color of your choice until end of turn.",
    r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"You"}},"effects":[{"Untap":{"what":"This"}},{"Continuous":{"affected":"This","mods":["AddProtectionFromChosenColor"],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn protection_from_the_color_of_your_choice_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    let kind = if text.starts_with("Whenever") {
        ClauseKind::Triggered
    } else if text.starts_with('{') {
        ClauseKind::Activated
    } else {
        ClauseKind::Spell
    };
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::spell("Target creature gains protection from the color of your choice until end of turn.")]
#[case::your_creatures(
    "Creatures you control gain protection from the color of your choice until end of turn."
)]
#[case::creature_or_enchantment(
    "Target creature or enchantment you control gains protection from the color of your choice until end of turn."
)]
fn protection_from_a_chosen_color_also_reads_in_a_spell(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("{W}: ~ gains protection from the color of your choice until end of turn.")]
#[case(
    "{1}, Sacrifice ~: Target creature or enchantment you control gains protection from the color of your choice until end of turn."
)]
fn protection_from_a_chosen_color_reads_in_an_activation(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::attacks("Whenever ~ attacks, it gets +1/+1 until end of turn.", r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}"#)]
#[case::another_attacker(
    "Whenever a creature you control attacks, it gets +2/+2 until end of turn.",
    r#"{"Triggered":{"when":{"Attacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn a_pump_of_it_in_a_trigger_names_the_creature_of_the_event(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::becomes_blocked("Whenever ~ becomes blocked, it gets +2/+2 until end of turn.")]
#[case::blocks("Whenever ~ blocks, it gets +7/+7 until end of turn.")]
#[case::gains_a_keyword("Whenever ~ attacks, it gains flying until end of turn.")]
#[case::counted(
    "Whenever ~ attacks, it gets +1/+1 until end of turn for each other attacking creature."
)]
#[case::a_dragon_attacks(
    "Whenever a Dragon you control attacks, it gains double strike until end of turn."
)]
#[case::a_flier_enters(
    "Whenever a creature you control with flying enters, it gains haste until end of turn."
)]
#[case::a_creature_blocked(
    "Whenever a creature you control becomes blocked, it gets +1/+1 until end of turn."
)]
#[case::the_card_by_name("Whenever ~ attacks, ~ gets +1/+1 until end of turn.")]
fn a_pump_of_it_reads_for_each_event_of_a_creature(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::the_card_attacks("Whenever ~ attacks, put a +1/+1 counter on it.")]
#[case::enchanted_creature_attacks(
    "Whenever enchanted creature attacks, put a +1/+1 counter on it."
)]
#[case::equipped_creature_attacks("Whenever equipped creature attacks, put a +1/+1 counter on it.")]
#[case::two_counters("Whenever ~ attacks, put two +1/+1 counters on it.")]
#[case::another_enters("Whenever another creature you control enters, put a +1/+1 counter on it.")]
#[case::a_kind_of_counter("Whenever ~ becomes blocked, put a charge counter on it.")]
#[case::a_count_of_counters_on_it(
    "Whenever ~ attacks, if there are five or more charge counters on it, draw a card."
)]
#[case::sacrificed("When ~ enters, sacrifice it.")]
fn a_counter_put_on_it_in_the_trigger_of_a_creature_reads_and_writes_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::this_creature(
    "Whenever one or more +1/+1 counters are put on this creature, draw a card, then discard a card.",
    r#"{"Triggered":{"when":{"Batch":{"CountersPut":{"what":["This"],"counter":"+1/+1"}}},"effects":[{"Draw":{}},{"Discard":{}}]}}"#
)]
#[case::a_creature_you_control(
    "Whenever one or more counters are put on a creature you control, you gain 1 life.",
    r#"{"Triggered":{"when":{"Batch":{"CountersPut":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::a_named_kind(
    "Whenever one or more charge counters are put on ~, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"CountersPut":{"what":["This"],"counter":"charge"}}},"effects":[{"Draw":{}}]}}"#
)]
fn one_or_more_counters_put_on_is_a_batch_of_counter_events(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case(
    "Whenever one or more +1/+1 counters are put on another creature you control, you may put a +1/+1 counter on this creature."
)]
#[case(
    "Whenever one or more +1/+1 counters are put on a creature you control, put that many growth counters on ~."
)]
#[case(
    "Whenever one or more -1/-1 counters are put on a creature an opponent controls, you gain 1 life."
)]
fn the_batch_of_counters_reads_with_the_effects_after_it(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::has_trample(
    "Each creature you control with a +1/+1 counter on it has trample.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasCounter":"+1/+1"}]},"mods":[{"AddKeyword":"Trample"}]}}"#
)]
#[case::gets_a_bonus(
    "Each creature you control with a +1/+1 counter on it gets +1/+1.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasCounter":"+1/+1"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#
)]
fn a_group_with_a_counter_on_it_gets_a_static_ability(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::lifelink("Each creature you control with a +1/+1 counter on it has lifelink.")]
#[case::flying("Each creature you control with a +1/+1 counter on it has flying.")]
#[case::first_strike("Each creature you control with a +1/+1 counter on it has first strike.")]
#[case::every_creature("Each creature has trample.")]
fn each_creature_with_a_keyword_reads_in_the_singular(#[case] text: &str) {
    golden(text, ClauseKind::Static, false);
}

#[rstest]
#[case::no_duration("Target creature gains protection from the color of your choice.")]
#[case::another_wording(
    "Target creature gains protection from a color of your choice until end of turn."
)]
#[case::the_color_chosen(
    "Target creature gains protection from the chosen color until end of turn."
)]
#[case::two_colors(
    "Target creature gains protection from two colors of your choice until end of turn."
)]
#[case::of_an_opponents_choice(
    "Target creature gains protection from the color of an opponent's choice until end of turn."
)]
#[case::a_player("You gain protection from the color of your choice until end of turn.")]
#[case::a_player_by_name(
    "Target player gains protection from the color of your choice until end of turn."
)]
#[case::after_a_keyword(
    "Target creature gains flying and protection from the color of your choice until end of turn."
)]
fn near_misses_of_protection_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::a_static("Enchanted creature has protection from the color of your choice.")]
#[case::a_static_group("Creatures you control have protection from the color of your choice.")]
fn a_static_never_chooses_a_color(#[case] text: &str) {
    assert_eq!(static_ability(text), None, "{text}");
}

#[rstest]
#[case::it_without_a_duration("Whenever ~ attacks, it gets +1/+1.")]
#[case::it_when_the_card_dies("Whenever ~ dies, it gets +1/+1 until end of turn.")]
#[case::it_when_a_spell_is_cast("Whenever you cast a spell, it gets +1/+1 until end of turn.")]
#[case::another_blocker(
    "Whenever another creature you control blocks, it gets +1/+1 until end of turn."
)]
#[case::it_after_the_trigger_of_a_player("Whenever you draw a card, put a +1/+1 counter on it.")]
#[case::it_in_a_second_sentence_without_antecedent("Draw a card. It gets +1/+1 until end of turn.")]
#[case::it_leaves("Whenever ~ leaves the battlefield, put a +1/+1 counter on it.")]
fn near_misses_of_it_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::it_in_a_spell("It gets +1/+1 until end of turn.")]
#[case::it_in_a_spell_counter("Put a +1/+1 counter on it.")]
fn it_in_a_spell_has_no_antecedent(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::two_or_more("Whenever two or more +1/+1 counters are put on this creature, draw a card.")]
#[case::singular("Whenever one or more +1/+1 counter is put on this creature, draw a card.")]
#[case::you_put(
    "Whenever you put one or more +1/+1 counters on a creature you control, draw a card."
)]
#[case::no_kind_word("Whenever one or more are put on this creature, draw a card.")]
#[case::rules_counter("Whenever one or more stun counters are put on this creature, draw a card.")]
#[case::on_a_player("Whenever one or more poison counters are put on target player, draw a card.")]
fn near_misses_of_the_counter_batch_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::plural_of_counters("Creatures you control with +1/+1 counters on them have trample.")]
#[case::plural_agreement("Each creature you control with a +1/+1 counter on it have trample.")]
#[case::without_on_it("Each creature you control with a +1/+1 counter has trample.")]
#[case::two_counters("Each creature you control with two +1/+1 counters on it has trample.")]
#[case::a_chosen_kind("Each creature you control with two counters on it has trample.")]
#[case::no_counter_kind("Each creature you control with a stun counter on it has trample.")]
fn near_misses_of_a_noun_with_a_counter_stay_unread(#[case] text: &str) {
    assert_eq!(static_ability(text), None, "{text}");
}
