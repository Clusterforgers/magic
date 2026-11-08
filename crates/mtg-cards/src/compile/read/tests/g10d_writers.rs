//! Goal: a cost change of other spells for a time and "becomes the color or colors of your choice" are written as cards word them (the duration first, after who casts or last), and another duration, change, caster, spell kind, amount, colour choice or subject is not their text.
use super::*;

/// Whether one of the wordings the checker accepts for the ability is the text.
fn writes(json: &str, text: &str) -> bool {
    super::super::render::written_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

fn cost_spell(applies_to: &str, change: &str, mana: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Rule":{{"rule":{{"SpellCost":{{"applies_to":{applies_to},"change":"{change}","mana":"{mana}"}}}},"duration":"{duration}"}}}}]}}}}"#
    )
}

const ARTIFACTS: &str = r#"["Spell",{"Type":"Artifact"},{"ControlledBy":"You"}]"#;
const NONCREATURE_THEIRS: &str =
    r#"["Spell",{"Not":{"Type":"Creature"}},{"ControlledBy":"Opponents"}]"#;
const THEIRS: &str = r#"["Spell",{"ControlledBy":"Opponents"}]"#;

#[rstest]
#[case::armor_wars(
    cost_spell(ARTIFACTS, "Reduce", "{1}", "UntilEndOfTurn"),
    "Artifact spells you cast this turn cost {1} less to cast.",
    true
)]
#[case::tax_collector(
    cost_spell(THEIRS, "Increase", "{1}", "UntilYourNextTurn"),
    "Until your next turn, spells your opponents cast cost {1} more to cast.",
    true
)]
#[case::elspeth_conquers_death(
    cost_spell(NONCREATURE_THEIRS, "Increase", "{2}", "UntilYourNextTurn"),
    "Noncreature spells your opponents cast cost {2} more to cast until your next turn.",
    true
)]
#[case::other_duration(
    cost_spell(ARTIFACTS, "Reduce", "{1}", "UntilYourNextTurn"),
    "Artifact spells you cast this turn cost {1} less to cast.",
    false
)]
#[case::other_change(
    cost_spell(ARTIFACTS, "Increase", "{1}", "UntilEndOfTurn"),
    "Artifact spells you cast this turn cost {1} less to cast.",
    false
)]
#[case::other_amount(
    cost_spell(ARTIFACTS, "Reduce", "{2}", "UntilEndOfTurn"),
    "Artifact spells you cast this turn cost {1} less to cast.",
    false
)]
#[case::other_caster(
    cost_spell(
        r#"["Spell",{"Type":"Artifact"},{"ControlledBy":"Opponents"}]"#,
        "Reduce",
        "{1}",
        "UntilEndOfTurn"
    ),
    "Artifact spells you cast this turn cost {1} less to cast.",
    false
)]
#[case::other_kind(
    cost_spell(THEIRS, "Increase", "{2}", "UntilYourNextTurn"),
    "Noncreature spells your opponents cast cost {2} more to cast until your next turn.",
    false
)]
fn a_cost_change_for_a_time(#[case] json: String, #[case] text: &str, #[case] same: bool) {
    assert_eq!(writes(&json, text), same);
}

fn paint(kind: &str, affected: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"ChooseValue":{{"kind":"{kind}","as":"chosen"}}}},{{"Continuous":{{"affected":{affected},"duration":"{duration}","mods":[{{"SetColorsChosen":"chosen"}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::quickchange(
    paint("Colors", r#"{"Target":0}"#, "UntilEndOfTurn"),
    "Target creature becomes the color or colors of your choice until end of turn.",
    true
)]
#[case::lasting(
    paint("Colors", r#"{"Target":0}"#, "Forever"),
    "Target creature becomes the color or colors of your choice.",
    true
)]
#[case::one_color_only(
    paint("Color", r#"{"Target":0}"#, "UntilEndOfTurn"),
    "Target creature becomes the color or colors of your choice until end of turn.",
    false
)]
#[case::other_duration(
    paint("Colors", r#"{"Target":0}"#, "Forever"),
    "Target creature becomes the color or colors of your choice until end of turn.",
    false
)]
#[case::other_subject(
    paint("Colors", r#""This""#, "UntilEndOfTurn"),
    "Target creature becomes the color or colors of your choice until end of turn.",
    false
)]
fn the_color_or_colors_of_your_choice(
    #[case] json: String,
    #[case] text: &str,
    #[case] same: bool,
) {
    assert_eq!(writes(&json, text), same);
}

#[rstest]
#[case::armor_wars(
    "Artifact spells you cast this turn cost {1} less to cast.",
    Some(cost_spell(ARTIFACTS, "Reduce", "{1}", "UntilEndOfTurn"))
)]
#[case::tax_collector(
    "Until your next turn, spells your opponents cast cost {1} more to cast.",
    Some(cost_spell(THEIRS, "Increase", "{1}", "UntilYourNextTurn"))
)]
#[case::elspeth(
    "Noncreature spells your opponents cast cost {2} more to cast until your next turn.",
    Some(cost_spell(NONCREATURE_THEIRS, "Increase", "{2}", "UntilYourNextTurn"))
)]
#[case::two_durations(
    "Until your next turn, spells your opponents cast cost {1} more to cast until your next turn.",
    None
)]
#[case::no_duration("Spells your opponents cast cost {1} more to cast.", None)]
fn a_cost_change_for_a_time_is_read(#[case] text: &str, #[case] expected: Option<String>) {
    assert_eq!(spell(text), expected.map(|json| ability(&json)));
}
