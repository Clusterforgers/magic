//! Goal: the shared noun-phrase reader turns each phrase with an open atom (modified, face-down, noncommander, a player or object target, put into a graveyard from the battlefield this turn, no counters, the chosen type or color, a stat compared with any amount) into one selector and the shared writer gives the same phrase back, under every determiner; the atoms work in whole clauses of effects and statics; near-misses stay unread.
use super::super::selectors::{Form, phrase, phrase_text};
use super::*;
use crate::refs::Selector;

/// Reads the whole phrase into a selector.
fn w21a_read(text: &str, form: Form) -> Option<Selector> {
    let tokens = tokenize(text);
    let mut reader = Reader::new(&tokens, false);
    let read = phrase(&mut reader, form)?;
    reader.end().then_some(read)
}

#[rstest]
#[case::modified(
    "modified creature",
    Form::Target,
    r#"[{"Type":"Creature"},"Modified"]"#
)]
#[case::modified_yours(
    "modified creatures you control",
    Form::All,
    r#"[{"Type":"Creature"},"Modified",{"ControlledBy":"You"}]"#
)]
#[case::other_modified(
    "other modified creatures you control",
    Form::All,
    r#"[{"Type":"Creature"},"Modified","Other",{"ControlledBy":"You"}]"#
)]
#[case::another_modified(
    "another modified creature you control",
    Form::Subject,
    r#"[{"Type":"Creature"},"Modified","Other",{"ControlledBy":"You"}]"#
)]
#[case::attacking_modified(
    "attacking modified creatures you control",
    Form::All,
    r#"[{"Type":"Creature"},"Attacking","Modified",{"ControlledBy":"You"}]"#
)]
#[case::modified_for_each(
    "modified creature you control",
    Form::Count,
    r#"[{"Type":"Creature"},"Modified",{"ControlledBy":"You"}]"#
)]
#[case::face_down(
    "face-down creatures you control",
    Form::All,
    r#"[{"Type":"Creature"},"FaceDown",{"ControlledBy":"You"}]"#
)]
#[case::face_down_one(
    "face-down creature",
    Form::Each,
    r#"[{"Type":"Creature"},"FaceDown"]"#
)]
#[case::tapped_face_down(
    "tapped face-down creature an opponent controls",
    Form::Target,
    r#"[{"Type":"Creature"},"Tapped","FaceDown",{"ControlledBy":"Opponents"}]"#
)]
#[case::noncommander(
    "noncommander creature you don't control",
    Form::Target,
    r#"[{"Type":"Creature"},{"Not":"Commander"},{"Not":{"ControlledBy":"You"}}]"#
)]
#[case::spell_targets_you(
    "spell that targets you",
    Form::Target,
    r#"["Spell",{"TargetsPlayer":"You"}]"#
)]
#[case::spell_targets_you_or_yours(
    "spell that targets you or a creature you control",
    Form::Target,
    r#"["Spell",{"AnyOf":[{"TargetsPlayer":"You"},{"Targeting":[{"Type":"Creature"},{"ControlledBy":"You"}]}]}]"#
)]
#[case::spell_targets_a_creature(
    "spell that targets a creature you control",
    Form::Target,
    r#"["Spell",{"Targeting":[{"Type":"Creature"},{"ControlledBy":"You"}]}]"#
)]
#[case::spell_targets_an_artifact(
    "spell that targets an artifact",
    Form::Each,
    r#"["Spell",{"Targeting":[{"Type":"Artifact"}]}]"#
)]
#[case::graved(
    "creature cards that were put into your graveyard from the battlefield this turn",
    Form::All,
    r#"[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"},"GravedThisTurn"]"#
)]
#[case::no_counters(
    "creatures with no counters on them",
    Form::All,
    r#"[{"Type":"Creature"},{"Not":"HasAnyCounter"}]"#
)]
#[case::no_counter_one(
    "creature with no counters on it",
    Form::Target,
    r#"[{"Type":"Creature"},{"Not":"HasAnyCounter"}]"#
)]
#[case::chosen_type(
    "creatures you control of the chosen type",
    Form::All,
    r#"[{"Type":"Creature"},{"ControlledBy":"You"},"ChosenSubtype"]"#
)]
#[case::chosen_type_token(
    "nontoken creature you control of the chosen type",
    Form::Subject,
    r#"[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"},"ChosenSubtype"]"#
)]
#[case::chosen_color(
    "creature of the chosen color",
    Form::Each,
    r#"[{"Type":"Creature"},"ChosenColor"]"#
)]
#[case::mana_value_le_count(
    "creature with mana value less than or equal to the number of lands you control",
    Form::Target,
    r#"[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]"#
)]
#[case::power_le_own(
    "creature with power less than or equal to ~'s power",
    Form::Target,
    r#"[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":{"Stat":{"of":"This","stat":"Power"}}}}]"#
)]
#[case::power_ge_own(
    "creatures with power greater than or equal to ~'s power",
    Form::All,
    r#"[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":{"Stat":{"of":"This","stat":"Power"}}}}]"#
)]
#[case::toughness_lt_power(
    "creature with toughness less than ~'s power",
    Form::Each,
    r#"[{"Type":"Creature"},{"Compare":{"stat":"Toughness","op":"Lt","to":{"Stat":{"of":"This","stat":"Power"}}}}]"#
)]
#[case::mana_value_gt_counters(
    "nonland permanent with mana value greater than the number of charge counters on ~",
    Form::Each,
    r#"[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Gt","to":{"Counters":{"on":"This","counter":"charge"}}}}]"#
)]
#[case::mana_value_eq_count(
    "creature with mana value equal to the number of Elves you control",
    Form::Target,
    r#"[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Eq","to":{"Count":[{"Subtype":"Elf"},{"ControlledBy":"You"}]}}}]"#
)]
fn a_phrase_with_an_open_atom_reads_and_writes_back(
    #[case] text: &str,
    #[case] form: Form,
    #[case] json: &str,
) {
    let read = w21a_read(text, form).unwrap_or_else(|| panic!("not read: {text}"));
    let expected: Selector = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
    assert_eq!(read, expected, "{text}");
    assert_eq!(phrase_text(&read, form).as_deref(), Some(text), "{text}");
}

#[rstest]
#[case::paired("creature it's paired with", Form::Target)]
#[case::targets_only("spell that targets only ~", Form::Target)]
#[case::targets_a_player("spell that targets a player", Form::Target)]
#[case::graved_elsewhere(
    "creature cards that were put into your graveyard from exile this turn",
    Form::All
)]
#[case::some_counters("creatures with two counters on them", Form::All)]
#[case::chosen_name("creature of the chosen name", Form::Each)]
#[case::compared_with_nothing("creature with power less than or equal to", Form::Target)]
#[case::compared_with_a_word("creature with power less than or equal to blue", Form::Target)]
#[case::at_most_amount(
    "creature with power at most the number of lands you control",
    Form::Target
)]
fn a_near_miss_of_an_atom_stays_unread(#[case] text: &str, #[case] form: Form) {
    assert_eq!(w21a_read(text, form), None, "{text}");
}

#[rstest]
#[case::grant_haste(
    "Modified creatures you control have haste.",
    ClauseKind::Static,
    r#"[{"Type":"Creature"},"Modified",{"ControlledBy":"You"}]"#
)]
#[case::face_down_pump(
    "Face-down creatures you control get +1/+1.",
    ClauseKind::Static,
    r#"[{"Type":"Creature"},"FaceDown",{"ControlledBy":"You"}]"#
)]
#[case::destroy_no_counters(
    "Destroy all creatures with no counters on them.",
    ClauseKind::Spell,
    r#"[{"Type":"Creature"},{"Not":"HasAnyCounter"}]"#
)]
#[case::counter_a_spell_at_you(
    "Counter target spell that targets you.",
    ClauseKind::Spell,
    r#"["Spell",{"TargetsPlayer":"You"}]"#
)]
fn a_clause_with_an_open_atom_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] selector: &str,
) {
    let read = clause(text, kind, false).unwrap_or_else(|| panic!("not read: {text}"));
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains(selector), "{text}: {json}");
    let wordings = render_variants(&read);
    assert!(
        wordings.iter().any(|written| same_text(written, text)),
        "{text}: {wordings:?}"
    );
}
