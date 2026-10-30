//! Goal: the static ability of a soulbond card, "As long as ~ is paired with another creature, each of those creatures (or both creatures) has / gets ...", reads into one `Static` over the pair under the condition `Paired`, writes back as the card's own text, and the near-misses stay with the table.
use super::*;

const PAIR: &str = r#"{"Each":[{"AnyOf":["This",{"PairedWith":"This"}]}]}"#;
const PAIRED: &str = r#"{"Matches":{"subject":"This","selector":["Paired"]}}"#;

#[rstest]
#[case::each_of_those_creatures_has_a_quoted_trigger(
    r#"As long as ~ is paired with another creature, each of those creatures has "Whenever this creature deals damage to an opponent, draw a card.""#,
    r#"[{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"Opponents"}},"effects":[{"Draw":{}}]}}}]"#
)]
#[case::both_creatures_have_a_keyword(
    "As long as ~ is paired with another creature, both creatures have double strike.",
    r#"[{"AddKeyword":"DoubleStrike"}]"#
)]
#[case::each_of_those_creatures_gets_a_bonus(
    "As long as ~ is paired with another creature, each of those creatures gets +4/+4.",
    r#"[{"ModifyPT":{"power":4,"toughness":4}}]"#
)]
fn the_pair_under_the_paired_condition_reads_and_writes_back(
    #[case] text: &str,
    #[case] mods: &str,
) {
    let json = format!(r#"{{"Static":{{"affected":{PAIR},"mods":{mods},"condition":{PAIRED}}}}}"#);
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::another_subject(
    "As long as ~ is paired with another creature, creatures you control have double strike."
)]
#[case::not_paired_with_a_creature("As long as ~ is paired, both creatures have double strike.")]
#[case::those_creatures_without_the_pairing("Each of those creatures has double strike.")]
#[case::paired_with_a_noncreature(
    "As long as ~ is paired with another permanent, both creatures have double strike."
)]
fn near_misses_do_not_read_as_the_pair(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    assert!(
        read.as_ref()
            .is_none_or(|ability| !format!("{ability:?}").contains("PairedWith")),
        "{text}: {read:?}"
    );
}
