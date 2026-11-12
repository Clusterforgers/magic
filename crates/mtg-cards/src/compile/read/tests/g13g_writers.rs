//! Goal: round 13 pack g13g writes the cost change of the equip and cycling abilities a player activates as Fluctuator, Bureau Headmaster, Auriok Steelshaper and Bladehold War-Whip print it; another kind, source, count or floor does not read back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn modifier(kind: &str, applies_to: &str, change: &str, mana: &str, rest: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"activating":"{kind}","applies_to":{applies_to},"change":"{change}","mana":"{mana}"{rest}}}}}"#
    )
}

const YOURS: &str = r#"[{"ControlledBy":"You"}]"#;
const OTHER_EQUIPMENT: &str = r#"[{"Subtype":"Equipment"},"Other",{"ControlledBy":"You"}]"#;

#[rstest]
#[case::fluctuator(
    modifier("Cycling", YOURS, "Reduce", "{2}", ""),
    "Cycling abilities you activate cost {2} less to activate.",
    true
)]
#[case::r301c_silver_fur_master(
    modifier("Ninjutsu", YOURS, "Reduce", "{1}", ""),
    "Ninjutsu abilities you activate cost {1} less to activate.",
    true
)]
#[case::r301c_ninjutsu_is_not_cycling(
    modifier("Ninjutsu", YOURS, "Reduce", "{1}", ""),
    "Cycling abilities you activate cost {1} less to activate.",
    false
)]
#[case::bureau_headmaster(
    modifier("Equip", YOURS, "Reduce", "{1}", ""),
    "Equip abilities you activate cost {1} less to activate.",
    true
)]
#[case::auriok_steelshaper(
    modifier("Equip", YOURS, "Reduce", "{1}", ""),
    "Equip costs you pay cost {1} less.",
    true
)]
#[case::war_whip_other_equipment(
    modifier("Equip", OTHER_EQUIPMENT, "Reduce", "{1}", ""),
    "Equip abilities you activate of other Equipment cost {1} less to activate.",
    true
)]
#[case::a_tax(
    modifier("Equip", YOURS, "Increase", "{1}", ""),
    "Equip abilities you activate cost {1} more to activate.",
    true
)]
#[case::reduce_written_as_a_tax(
    modifier("Equip", YOURS, "Reduce", "{1}", ""),
    "Equip abilities you activate cost {1} more to activate.",
    false
)]
#[case::another_amount(
    modifier("Cycling", YOURS, "Reduce", "{1}", ""),
    "Cycling abilities you activate cost {2} less to activate.",
    false
)]
#[case::another_kind(
    modifier("Cycling", YOURS, "Reduce", "{2}", ""),
    "Equip abilities you activate cost {2} less to activate.",
    false
)]
#[case::opponents_abilities(
    modifier("Equip", r#"[{"ControlledBy":"Opponents"}]"#, "Reduce", "{1}", ""),
    "Equip abilities you activate cost {1} less to activate.",
    false
)]
#[case::a_floor(
    modifier("Equip", YOURS, "Reduce", "{1}", r#","least":1"#),
    "Equip abilities you activate cost {1} less to activate.",
    false
)]
#[case::other_equipment_needs_the_equipment(
    modifier(
        "Equip",
        r#"[{"Subtype":"Equipment"},{"ControlledBy":"You"}]"#,
        "Reduce",
        "{1}",
        ""
    ),
    "Equip abilities you activate of other Equipment cost {1} less to activate.",
    false
)]
fn an_activation_cost_change_reads_back_as_printed(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{clause}");
}
