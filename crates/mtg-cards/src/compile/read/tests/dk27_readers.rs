//! Goal: a player's sacrifices or discards of one effect read as one batch ("Whenever you sacrifice one or more Foods"),
//! "remove X counters from among creatures you control" reads as a cost of the payer's own permanents, and "you may
//! <verb> any number of ... This spell costs {N} less to cast for each ... this way." reads as the required X cost and the
//! spell's own reduction by X; each writes back as the printed text, and a near miss reads as something else or not at all.
use super::*;
use crate::compile::read::{Own, static_pair, static_pair_texts};

fn writes(json: &str, text: &str) -> bool {
    render_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, text))
}

#[rstest]
#[case::camellia(
    "Whenever you sacrifice one or more Foods, create a 1/1 green Squirrel creature token.",
    r#"{"Triggered":{"when":{"Batch":{"Sacrificed":{"what":[{"Subtype":"Food"}],"by":"You"}}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Squirrel","colors":["Green"],"types":["Creature"],"subtypes":["Squirrel"],"power":1,"toughness":1}}}}]}}"#
)]
#[case::a_player_sacrifices(
    "Whenever a player sacrifices one or more creatures, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"Sacrificed":{"what":[{"Type":"Creature"}]}}},"effects":[{"Draw":{}}]}}"#
)]
#[case::you_discard(
    "Whenever you discard one or more cards, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"Discarded":{"by":"You"}}},"effects":[{"Draw":{}}]}}"#
)]
#[case::a_player_discards_lands(
    "Whenever a player discards one or more land cards, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"Discarded":{"what":[{"Type":"Land"}]}}},"effects":[{"Draw":{}}]}}"#
)]
fn one_or_more_reads_as_a_batch(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::one_food("Whenever you sacrifice a Food, draw a card.", "Batch")]
#[case::one_card("Whenever you discard a card, draw a card.", "Batch")]
fn a_single_event_is_no_batch(#[case] text: &str, #[case] word: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    assert!(
        !serde_json::to_string(&read).unwrap().contains(word),
        "{text}"
    );
}

#[rstest]
#[case::two_or_more("Whenever you sacrifice two or more Foods, draw a card.")]
#[case::discard_creatures("Whenever you discard one or more creatures, draw a card.")]
#[case::discard_two_types("Whenever you discard one or more artifact or land cards, draw a card.")]
fn a_near_miss_of_the_batch_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

const TITAN_COST: &str = r#"{"AdditionalCost":{"cost":[{"RemoveCountersAmong":{"counter":"+1/+1","count":"X","from":[{"Type":"Creature"}]}}],"required":true}}"#;
const LESS: &str =
    r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}","per":"X"}}"#;
const TITAN: &str = "As an additional cost to cast this spell, you may remove any number of +1/+1 counters from among creatures you control. This spell costs {2} less to cast for each counter removed this way.";
const EXTUS: &str = "As an additional cost to cast this spell, you may sacrifice any number of creatures. This spell costs {2} less to cast for each creature sacrificed this way.";

#[rstest]
#[case::hierophant(TITAN, TITAN_COST)]
#[case::awaken_the_blood_avatar(
    EXTUS,
    r#"{"AdditionalCost":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}],"count":"X"}}],"required":true}}"#
)]
fn any_number_and_less_for_each_reads_as_the_x_cost_and_its_reduction(
    #[case] text: &str,
    #[case] cost: &str,
) {
    let read = static_pair(text, false, &Own::default()).expect(text);
    assert_eq!(read, vec![ability(cost), ability(LESS)], "{text}");
}

#[rstest]
#[case::another_noun(
    "As an additional cost to cast this spell, you may remove any number of +1/+1 counters from among creatures you control. This spell costs {2} less to cast for each creature sacrificed this way."
)]
#[case::opponents_creatures(
    "As an additional cost to cast this spell, you may remove any number of +1/+1 counters from among creatures your opponents control. This spell costs {2} less to cast for each counter removed this way."
)]
#[case::more_instead(
    "As an additional cost to cast this spell, you may remove any number of +1/+1 counters from among creatures you control. This spell costs {2} more to cast for each counter removed this way."
)]
#[case::no_may(
    "As an additional cost to cast this spell, remove any number of +1/+1 counters from among creatures you control. This spell costs {2} less to cast for each counter removed this way."
)]
fn a_near_miss_of_the_pair_stays_unread(#[case] text: &str) {
    assert_eq!(static_pair(text, false, &Own::default()), None, "{text}");
}

#[rstest]
#[case::three_less(TITAN_COST, LESS, &TITAN.replace("{2} less", "{3} less"))]
#[case::a_fixed_count(
    &TITAN_COST.replace(r#""count":"X""#, r#""count":2"#),
    LESS,
    TITAN
)]
#[case::any_kind(&TITAN_COST.replace(r#""counter":"+1/+1","#, ""), LESS, TITAN)]
#[case::per_card(
    TITAN_COST,
    r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}","per":{"Count":[{"Type":"Creature"}]}}}"#,
    TITAN
)]
fn a_near_miss_of_the_rules_is_not_written_as_the_pair(
    #[case] cost: &str,
    #[case] less: &str,
    #[case] text: &str,
) {
    let written = static_pair_texts(&ability(cost), &ability(less));
    assert!(
        !written.iter().any(|said| same_text(said, text)),
        "{written:?}"
    );
}

#[rstest]
#[case::among_yours(
    "As an additional cost to cast this spell, remove X +1/+1 counters from among creatures you control.",
    TITAN_COST
)]
fn removing_counters_from_among_your_permanents_reads(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, true, json);
}

#[rstest]
#[case::with_you_control_in_the_filter(
    r#"{"AdditionalCost":{"cost":[{"RemoveCountersAmong":{"counter":"+1/+1","count":"X","from":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"required":true}}"#
)]
#[case::from_nothing(
    r#"{"AdditionalCost":{"cost":[{"RemoveCountersAmong":{"counter":"+1/+1","count":"X","from":[]}}],"required":true}}"#
)]
fn a_cost_from_among_nothing_or_said_twice_is_not_written(#[case] json: &str) {
    assert!(
        !writes(
            json,
            "As an additional cost to cast ~, remove X +1/+1 counters from among creatures you control."
        ),
        "{json}"
    );
}
