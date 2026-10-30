//! Goal: the condition "this Aura entered this turn" is the card itself entering the battlefield this turn, and a static ability given in quotes that gives keywords to what its holder is attached to ("Equipped creature has lifelink.") reads and writes back; a quote with a condition, a bonus or a group, and another noun or time, stay unread.
use super::*;
use crate::compile::read::same_text;

#[rstest]
#[case::this_aura(
    "As long as this Aura entered this turn, enchanted creature has hexproof.",
    r#""mods":[{"AddKeyword":"Hexproof"}]"#
)]
#[case::the_card_by_name(
    "As long as ~ entered this turn, enchanted creature has hexproof.",
    r#""mods":[{"AddKeyword":"Hexproof"}]"#
)]
#[case::on_the_battlefield(
    "As long as this Aura entered the battlefield this turn, enchanted creature has first strike.",
    r#""mods":[{"AddKeyword":"FirstStrike"}]"#
)]
fn an_aura_that_entered_this_turn_is_a_condition_of_the_card_itself(
    #[case] text: &str,
    #[case] mods: &str,
) {
    let read =
        clause(text, ClauseKind::Static, false).unwrap_or_else(|| panic!("not read: {text}"));
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(mods), "{json}");
    assert!(
        json.contains(
            r#""condition":{"Matches":{"subject":"This","selector":["EnteredThisTurn"]}}"#
        ),
        "{json}"
    );
    let written = render_variants(&read);
    assert!(
        written.iter().any(|variant| same_text(variant, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::lifelink_for_an_equipment(
    "As long as enchanted permanent is an Equipment, it has \"Equipped creature has lifelink.\"",
    r#"{"AddKeyword":"Lifelink"}"#
)]
#[case::two_keywords(
    "As long as enchanted permanent is an Equipment, it has \"Equipped creature has flying and lifelink.\"",
    r#"{"AddKeyword":"Flying"},{"AddKeyword":"Lifelink"}"#
)]
#[case::protection(
    "As long as enchanted permanent is an Equipment, it has \"Equipped creature has protection from red.\"",
    r#"{"AddKeyword":{"Protection":[{"Color":"Red"}]}}"#
)]
fn a_static_ability_given_in_quotes_reads_and_writes_back(#[case] text: &str, #[case] mods: &str) {
    let read = golden(text, ClauseKind::Static, false);
    let json = serde_json::to_string(&read).unwrap();
    let granted = format!(r#""GrantAbility":{{"Static":{{"affected":"Attached","mods":[{mods}]"#);
    assert!(json.contains(&granted), "{json}");
}

#[rstest]
#[case::a_condition_inside(
    "As long as enchanted permanent is an Equipment, it has \"As long as it's tapped, equipped creature has lifelink.\""
)]
#[case::a_bonus_inside(
    "As long as enchanted permanent is an Equipment, it has \"Equipped creature gets +1/+1.\""
)]
#[case::a_group_inside(
    "As long as enchanted permanent is an Equipment, it has \"Creatures you control have lifelink.\""
)]
#[case::entered_for_another_time(
    "As long as this Aura entered last turn, enchanted creature has hexproof."
)]
fn another_quote_or_condition_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
