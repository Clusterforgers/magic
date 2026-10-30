//! Goal: the triggers of the deck's creatures read and write as their cards say: a spell that targets ~ ("Heroic"), "it connives" after the card itself becomes the target of a spell, and every other object, player, count or event stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

/// Whether some wording the checker accepts for the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

const COUNTER: &str = r#"[{"PutCounters":{"on":"This","counter":"+1/+1"}}]"#;
const CONNIVES: &str = r#"[{"Connive":{"what":"This"}}]"#;

fn cast(what: &str, by: &str, effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":{what},"by":{by}}}}},"effects":{effects}}}}}"#
    )
}

fn becomes_target(by: &str, effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"BecomesTarget":{{"what":["This"],"by":{by}}}}},"effects":{effects}}}}}"#
    )
}

#[rstest]
#[case::heroic(
    "Whenever you cast a spell that targets ~, put a +1/+1 counter on ~.",
    r#"[{"Targeting":["This"]}]"#
)]
#[case::this_creature(
    "Whenever you cast a spell that targets this creature, put a +1/+1 counter on this creature.",
    r#"[{"Targeting":["This"]}]"#
)]
#[case::a_creature_spell(
    "Whenever you cast a creature spell that targets ~, put a +1/+1 counter on ~.",
    r#"[{"Type":"Creature"},{"Targeting":["This"]}]"#
)]
fn a_spell_that_targets_the_card_reads_and_writes(#[case] text: &str, #[case] what: &str) {
    reads_as(
        text,
        ClauseKind::Triggered,
        false,
        &cast(what, r#""You""#, COUNTER),
    );
}

#[rstest]
#[case::another_object(
    "Whenever you cast a spell that targets another creature, put a +1/+1 counter on ~."
)]
#[case::a_player("Whenever you cast a spell that targets a player, put a +1/+1 counter on ~.")]
#[case::any_spell("Whenever you activate an ability that targets ~, put a +1/+1 counter on ~.")]
#[case::a_target_of_nothing("Whenever you cast a spell that targets, put a +1/+1 counter on ~.")]
fn near_misses_of_the_spell_that_targets_the_card_stay_with_the_table(#[case] text: &str) {
    let heroic = cast(r#"[{"Targeting":["This"]}]"#, r#""You""#, COUNTER);
    assert_ne!(trigger(text), Some(ability(&heroic)), "{text}");
    assert!(!writes(&heroic, text), "{text}");
}

#[rstest]
#[case::virtuoso(
    "Whenever this creature becomes the target of a spell you control, it connives.",
    r#"["Spell",{"ControlledBy":"You"}]"#
)]
#[case::named(
    "Whenever ~ becomes the target of a spell you control, it connives.",
    r#"["Spell",{"ControlledBy":"You"}]"#
)]
fn the_card_connives_when_a_spell_it_names_targets_it_writes_it(
    #[case] text: &str,
    #[case] by: &str,
) {
    assert!(writes(&becomes_target(by, CONNIVES), text), "{text}");
}

#[rstest]
#[case::another_spell_owner(
    "Whenever this creature becomes the target of a spell you control, it connives.",
    r#"["Spell",{"ControlledBy":"Opponents"}]"#
)]
#[case::any_spell(
    "Whenever this creature becomes the target of a spell you control, it connives.",
    r#"["Spell"]"#
)]
#[case::anything_you_control(
    "Whenever this creature becomes the target of a spell you control, it connives.",
    r#"[{"ControlledBy":"You"}]"#
)]
fn a_connive_after_another_event_is_not_the_text(#[case] text: &str, #[case] by: &str) {
    assert!(!writes(&becomes_target(by, CONNIVES), text), "{text}");
}
