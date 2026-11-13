//! Goal: "You may pay {0} rather than pay the echo cost for permanents you control." reads as the rule `EchoRather` of its cost and "As ~ enters, an opponent chooses a creature type." as the replacement `OpponentChooses` of its kind, each written back as the same text; another keyword cost, another group of permanents, a missing "may", another chooser or another kind stay unread; "As this Equipment becomes attached to a creature, choose a color." reads as `BecomesAttached` with `Choose` and its static writes "the last chosen color", while another host, kind or verb stays unread.
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

#[rstest]
#[case::a_color(
    "As this Equipment becomes attached to a creature, choose a color.",
    r#"{"Replacement":{"when":{"BecomesAttached":{"what":["This"]}},"by":{"Choose":"Color"}}}"#
)]
#[case::a_creature_type(
    "As this Equipment becomes attached to a creature, choose a creature type.",
    r#"{"Replacement":{"when":{"BecomesAttached":{"what":["This"]}},"by":{"Choose":"CreatureType"}}}"#
)]
fn r5501_a_choice_as_it_becomes_attached_reads_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::a_permanent("As this Equipment becomes attached to a permanent, choose a color.")]
#[case::a_player("As this Equipment becomes attached to a creature, choose a player.")]
#[case::unattached("As this Equipment becomes unattached from a creature, choose a color.")]
fn r5501_another_attaching_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[test]
fn r5501_the_last_chosen_color_of_an_equipment_writes_back() {
    let ability = ability(
        r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":0}},{"AddKeyword":{"Protection":["ChosenColor"]}}]}}"#,
    );
    let wordings = render_variants(&ability);
    assert!(
        wordings.iter().any(|written| same_text(
            written,
            "Equipped creature gets +2/+0 and has protection from the last chosen color."
        )),
        "{wordings:?}"
    );
}
