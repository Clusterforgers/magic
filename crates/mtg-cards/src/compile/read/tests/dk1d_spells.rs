//! Goal: the spells of the owner's deck write back as their printed text: a group that is not enchanted, a pump of the enchanted creature as an Aura enters, and a counter that the promised gift retargets; the near-misses are not written.
use super::*;

/// Whether the renderers write the ability as the text, in one of their wordings.
fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

#[rstest]
#[case::all_creatures_not_enchanted(
    "Destroy all creatures that aren't enchanted. They can't be regenerated.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"HasAttached":[{"Subtype":"Aura"}]}}]},"regenerate":false}}]}}"#
)]
#[case::all_creatures_not_enchanted_and_regenerable(
    "Destroy all creatures that aren't enchanted.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"HasAttached":[{"Subtype":"Aura"}]}}]}}}]}}"#
)]
fn a_group_that_is_not_enchanted_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::enchanted_by_an_aura_you_control(
    "Destroy all creatures that aren't enchanted by an Aura you control.",
    ClauseKind::Spell
)]
#[case::equipped("Destroy all creatures that aren't equipped.", ClauseKind::Spell)]
#[case::the_wrong_number("Destroy all creatures that isn't enchanted.", ClauseKind::Spell)]
#[case::an_aura_alone("Destroy all creatures that aren't Auras.", ClauseKind::Spell)]
fn a_group_that_is_not_enchanted_stays_with_the_table_for_other_wordings(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    let read = clause(text, kind, false);
    assert!(
        read.is_none_or(|read| !render_variants(&read)
            .iter()
            .any(|written| same_text(written, "Destroy all creatures that aren't enchanted."))),
        "{text}"
    );
}

#[rstest]
#[case::hexproof_as_the_aura_enters(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":"Attached","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Hexproof"}]}}]}}"#,
    "When this Aura enters, enchanted creature gains hexproof until end of turn."
)]
#[case::flying_as_the_aura_enters(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":"Attached","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Flying"}]}}]}}"#,
    "When this Aura enters, enchanted creature gains flying until end of turn."
)]
#[case::a_pump_of_the_enchanted_creature(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":"Attached","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":2}}]}}]}}"#,
    "When this Aura enters, enchanted creature gets +2/+2 until end of turn."
)]
fn the_enchanted_creature_gains_until_end_of_turn(#[case] json: &str, #[case] text: &str) {
    assert!(writes(json, text), "{text}");
}

#[rstest]
#[case::a_longer_duration_is_not_until_end_of_turn(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":"Attached","duration":"UntilYourNextTurn","mods":[{"AddKeyword":"Hexproof"}]}}]}}"#,
    "When this Aura enters, enchanted creature gains hexproof until end of turn."
)]
#[case::the_source_is_not_the_enchanted_creature(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Hexproof"}]}}]}}"#,
    "When this Aura enters, enchanted creature gains hexproof until end of turn."
)]
fn the_enchanted_creature_wording_is_not_written_for_other_rules(
    #[case] json: &str,
    #[case] text: &str,
) {
    assert!(!writes(json, text), "{text}");
}

const GIFT_TARGETS: &str = r#""targets":[{"what":{"Object":["Spell",{"Type":"Creature"}]},"bounds":{"min":1,"max":{"Sum":[1,{"Times":{"by":-1,"of":"Promised"}}]}}},{"what":{"Object":["Spell"]},"bounds":{"min":1,"max":"Promised"}}]"#;

#[rstest]
#[case::counter_a_creature_spell_or_any_with_the_gift(
    r#""effects":[{"If":{"condition":"AdditionalCostPaid","then":[{"Counter":{"what":{"Target":1}}}],"otherwise":[{"Counter":{"what":{"Target":0}}}]}}]"#,
    true
)]
#[case::the_targets_swapped(
    r#""effects":[{"If":{"condition":"AdditionalCostPaid","then":[{"Counter":{"what":{"Target":0}}}],"otherwise":[{"Counter":{"what":{"Target":1}}}]}}]"#,
    false
)]
#[case::another_condition(
    r#""effects":[{"If":{"condition":"YourTurn","then":[{"Counter":{"what":{"Target":1}}}],"otherwise":[{"Counter":{"what":{"Target":0}}}]}}]"#,
    false
)]
#[case::an_effect_after_the_choice(
    r#""effects":[{"If":{"condition":"AdditionalCostPaid","then":[{"Counter":{"what":{"Target":1}}}],"otherwise":[{"Counter":{"what":{"Target":0}}}]}},{"Draw":{}}]"#,
    false
)]
#[case::no_otherwise(
    r#""effects":[{"If":{"condition":"AdditionalCostPaid","then":[{"Counter":{"what":{"Target":1}}}]}}]"#,
    false
)]
fn a_counter_that_the_promised_gift_retargets_is_written_only_in_its_shape(
    #[case] effects: &str,
    #[case] written: bool,
) {
    let json = format!(r#"{{"Spell":{{{GIFT_TARGETS},{effects}}}}}"#);
    let text =
        "Counter target creature spell. If the gift was promised, instead counter target spell.";
    assert_eq!(writes(&json, text), written, "{json}");
}

#[rstest]
#[case::counts_that_do_not_depend_on_the_gift(
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Type":"Creature"}]}},{"what":{"Object":["Spell"]}}],"effects":[{"If":{"condition":"AdditionalCostPaid","then":[{"Counter":{"what":{"Target":1}}}],"otherwise":[{"Counter":{"what":{"Target":0}}}]}}]}}"#
)]
#[case::the_counts_swapped(
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Type":"Creature"}]},"bounds":{"min":1,"max":"Promised"}},{"what":{"Object":["Spell"]},"bounds":{"min":1,"max":{"Sum":[1,{"Times":{"by":-1,"of":"Promised"}}]}}}],"effects":[{"If":{"condition":"AdditionalCostPaid","then":[{"Counter":{"what":{"Target":1}}}],"otherwise":[{"Counter":{"what":{"Target":0}}}]}}]}}"#
)]
fn bounds_other_than_the_gift_are_not_written_as_the_gift(#[case] json: &str) {
    let text =
        "Counter target creature spell. If the gift was promised, instead counter target spell.";
    assert!(!writes(json, text), "{json}");
}
