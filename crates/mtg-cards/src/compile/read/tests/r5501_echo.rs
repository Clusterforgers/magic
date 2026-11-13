//! Goal: "You may pay {0} rather than pay the echo cost for permanents you control." reads as the rule `EchoRather` of its cost and "As ~ enters, an opponent chooses a creature type." as the replacement `OpponentChooses` of its kind, each written back as the same text; another keyword cost, another group of permanents, a missing "may", another chooser or another kind stay unread.
use super::*;

#[rstest]
#[case::zero(
    "You may pay {0} rather than pay the echo cost for permanents you control.",
    r#"{"Rule":{"rule":{"EchoRather":"{0}"}}}"#
)]
#[case::one_generic(
    "You may pay {1} rather than pay the echo cost for permanents you control.",
    r#"{"Rule":{"rule":{"EchoRather":"{1}"}}}"#
)]
#[case::a_colored_symbol(
    "You may pay {R} rather than pay the echo cost for permanents you control.",
    r#"{"Rule":{"rule":{"EchoRather":"{R}"}}}"#
)]
fn r5501_another_cost_for_echo_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::another_keyword_cost(
    "You may pay {0} rather than pay the buyback cost for permanents you control."
)]
#[case::another_group("You may pay {0} rather than pay the echo cost for creatures you control.")]
#[case::opponents_permanents(
    "You may pay {0} rather than pay the echo cost for permanents your opponents control."
)]
#[case::without_may("Pay {0} rather than pay the echo cost for permanents you control.")]
#[case::no_cost("You may pay rather than pay the echo cost for permanents you control.")]
fn r5501_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::a_creature_type(
    "As ~ enters, an opponent chooses a creature type.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"OpponentChooses":"CreatureType"}}}"#
)]
#[case::a_color(
    "As ~ enters, an opponent chooses a color.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"OpponentChooses":"Color"}}}"#
)]
fn r5501_an_opponents_choice_as_it_enters_reads_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::each_opponent("As ~ enters, each opponent chooses a creature type.")]
#[case::target_opponent("As ~ enters, target opponent chooses a creature type.")]
#[case::a_player("As ~ enters, a player chooses a creature type.")]
#[case::another_kind("As ~ enters, an opponent chooses a card type.")]
fn r5501_another_chooser_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
