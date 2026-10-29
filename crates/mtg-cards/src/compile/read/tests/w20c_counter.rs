//! Goal: a spell or ability that targets something of yours is countered unless its controller pays, read from the sentence of a trigger of a target and written back as the card words it, and a near-miss stays with the table.
use super::*;

#[rstest]
#[case::a_group_of_slivers(
    "Whenever a Sliver creature you control becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {2}.",
    ClauseKind::Triggered
)]
#[case::the_card_itself(
    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {1}.",
    ClauseKind::Triggered
)]
#[case::two_colors(
    "Whenever a permanent you control becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {1}{U}.",
    ClauseKind::Triggered
)]
fn w20c_a_targeting_spell_is_countered_unless_it_is_paid_for(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    golden(text, kind, false);
}

#[test]
fn w20c_the_payment_reads_into_the_dsl() {
    let read = golden(
        "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {2}.",
        ClauseKind::Triggered,
        false,
    );
    let Ability::Triggered { effects, .. } = read else {
        panic!("not a triggered ability");
    };
    assert_eq!(
        serde_json::to_string(&effects).unwrap_or_default(),
        r#"[{"Pay":{"who":"Triggering","cost":"{2}","unpaid":[{"Counter":{"what":"Triggering"}}]}}]"#
    );
}

#[rstest]
#[case::in_a_spell(
    "Counter that spell or ability unless its controller pays {2}.",
    ClauseKind::Spell
)]
#[case::you_pay(
    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell or ability unless you pay {2}.",
    ClauseKind::Triggered
)]
#[case::no_cost(
    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays.",
    ClauseKind::Triggered
)]
#[case::life_not_mana(
    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays 2 life.",
    ClauseKind::Triggered
)]
#[case::an_x_cost(
    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {X}.",
    ClauseKind::Triggered
)]
#[case::another_object(
    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell unless its controller pays {2}.",
    ClauseKind::Triggered
)]
fn w20c_a_counter_near_miss_stays_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
