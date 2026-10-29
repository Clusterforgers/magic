//! Goal: statics over a group named by an attachment ("Enchanted creatures you control get +2/+2", "Equipped Warriors you control have double strike") or a list of creature types, and counts of "each Aura attached to it" and "each of its colors", read into the rules, write back as the same text, and leave every near-miss (a single host, another kind, a lone type, a foreign count) unread and unwritten.
use super::*;

const CREATURE: &str = r#"{"Type":"Creature"}"#;
const YOURS: &str = r#"{"ControlledBy":"You"}"#;
const AURA: &str = r#"{"HasAttached":[{"Subtype":"Aura"}]}"#;
const EQUIPMENT: &str = r#"{"HasAttached":[{"Subtype":"Equipment"}]}"#;

fn group(preds: &str, mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":{{"Each":[{preds}]}},"mods":[{mods}]}}}}"#)
}

fn host(affected: &str, mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":{affected},"mods":[{mods}]}}}}"#)
}

fn plus(power: &str, toughness: &str) -> String {
    format!(r#"{{"ModifyPT":{{"power":{power},"toughness":{toughness}}}}}"#)
}

/// The count of Auras attached to the host.
fn auras_on(host: &str) -> String {
    format!(r#"{{"Count":[{{"Subtype":"Aura"}},{{"AttachedTo":"{host}"}}]}}"#)
}

#[rstest]
#[case::enchanted_creatures_you_control(
    "Enchanted creatures you control get +2/+2.",
    &format!("{CREATURE},{AURA},{YOURS}"),
    &plus("2", "2")
)]
#[case::enchanted_creatures_with_a_keyword(
    "Enchanted creatures you control have shroud.",
    &format!("{CREATURE},{AURA},{YOURS}"),
    r#"{"AddKeyword":"Shroud"}"#
)]
#[case::equipped_creatures(
    "Equipped creatures you control get +1/+1.",
    &format!("{CREATURE},{EQUIPMENT},{YOURS}"),
    &plus("1", "1")
)]
#[case::equipped_warriors(
    "Equipped Warriors you control have double strike.",
    &format!(r#"{{"Subtype":"Warrior"}},{EQUIPMENT},{YOURS}"#),
    r#"{"AddKeyword":"DoubleStrike"}"#
)]
#[case::every_enchanted_creature(
    "Each enchanted creature you control gets +1/+1.",
    &format!("{CREATURE},{AURA},{YOURS}"),
    &plus("1", "1")
)]
fn a_group_named_by_its_attachment(
    #[case] text: &str,
    #[case] group_preds: &str,
    #[case] mods: &str,
) {
    reads_as(text, ClauseKind::Static, false, &group(group_preds, mods));
}

#[rstest]
#[case::lone_host_is_not_a_group("Enchanted creature you control gets +2/+2.")]
#[case::another_attachment("Attached creatures you control get +2/+2.")]
#[case::fortified("Fortified lands you control have hexproof.")]
#[case::unknown_noun("Enchanted Pickles you control get +1/+1.")]
fn a_near_miss_of_an_attached_group_stays_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert!(
        read.is_none_or(|read| !render_variants(&read).is_empty()
            && !format!("{read:?}").contains("HasAttached")),
        "{text}"
    );
}

#[rstest]
#[case::barbarian_warrior_berserker(
    "Each creature that's a Barbarian, a Warrior, or a Berserker gets +2/+2 and has haste.",
    &["Barbarian", "Warrior", "Berserker"]
)]
#[case::two_types(
    "Each creature that's a Goblin or a Warrior gets +2/+2 and has haste.",
    &["Goblin", "Warrior"]
)]
fn a_group_named_by_a_list_of_creature_types(#[case] text: &str, #[case] kinds: &[&str]) {
    let list: Vec<String> = kinds
        .iter()
        .map(|kind| format!(r#"{{"Subtype":"{kind}"}}"#))
        .collect();
    let mods = format!(r#"{},{{"AddKeyword":"Haste"}}"#, plus("2", "2"));
    let preds = format!(r#"{CREATURE},{{"AnyOf":[{}]}}"#, list.join(","));
    reads_as(text, ClauseKind::Static, false, &group(&preds, &mods));
}

#[rstest]
#[case::one_type("Each creature that's a Goblin gets +2/+2.")]
#[case::no_article("Each creature that's Goblin, Warrior, or Berserker gets +2/+2.")]
#[case::not_a_type("Each creature that's a Barbarian, a Warrior, or a Pickle gets +2/+2.")]
fn a_near_miss_of_a_list_of_types_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::the_card_itself(
    "This creature gets +2/+2 for each Aura attached to it.",
    "\"This\"",
    &plus_each("2", "This")
)]
#[case::the_host(
    "Equipped creature gets +1/+1 for each Aura attached to it.",
    "\"Attached\"",
    &plus_each("1", "Attached")
)]
fn a_bonus_for_each_aura_attached(#[case] text: &str, #[case] affected: &str, #[case] mods: &str) {
    reads_as(text, ClauseKind::Static, false, &host(affected, mods));
}

fn plus_each(by: &str, host: &str) -> String {
    let count = auras_on(host);
    let power = if by == "1" {
        count
    } else {
        format!(r#"{{"Times":{{"by":{by},"of":{count}}}}}"#)
    };
    let toughness = power.clone();
    format!(r#"{{"ModifyPT":{{"power":{power},"toughness":{toughness}}}}}"#)
}

#[rstest]
#[case::the_host(
    "Enchanted creature gets +1/+1 for each of its colors.",
    "\"Attached\"",
    "Attached"
)]
#[case::the_equipped_host(
    "Equipped creature gets +1/+1 for each of its colors.",
    "\"Attached\"",
    "Attached"
)]
fn a_bonus_for_each_of_its_colors(#[case] text: &str, #[case] affected: &str, #[case] of: &str) {
    let colors = format!(r#"{{"Stat":{{"of":"{of}","stat":"Colors"}}}}"#);
    let mods = format!(r#"{{"ModifyPT":{{"power":{colors},"toughness":{colors}}}}}"#);
    let written = written_variants(&ability(&host(affected, &mods)));
    assert!(
        written.iter().any(|wording| same_text(wording, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::another_count("Enchanted creature gets +1/+1 for each of your colors.")]
#[case::a_spells_colors_on_a_static(
    "Enchanted creature gets +1/+1 for each of that spell's colors."
)]
fn a_foreign_count_of_colors_stays_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert!(
        read.is_none_or(|read| !format!("{read:?}").contains("Colors")
            || format!("{read:?}").contains("Triggering")),
        "{text}"
    );
}

#[rstest]
#[case::another_host_has_no_words(r#"{"Count":[{"Subtype":"Aura"},{"AttachedTo":"You"}]}"#)]
#[case::a_plain_count_is_on_the_battlefield(r#"{"Count":[{"Subtype":"Aura"}]}"#)]
fn a_count_that_is_not_attached_to_it_is_not_written_so(#[case] count: &str) {
    let mods = format!(r#"{{"ModifyPT":{{"power":{count},"toughness":{count}}}}}"#);
    let text = "Equipped creature gets +1/+1 for each Aura attached to it.";
    let written = written_variants(&ability(&host("\"Attached\"", &mods)));
    assert!(
        !written.iter().any(|wording| same_text(wording, text)),
        "{written:?}"
    );
}
