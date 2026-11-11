//! Goal: the rmt4 wordings ("When this creature enters, if it was bargained, ...") read a Bargain card's enters trigger back to the same rule, and a trigger of another condition or event still reads Differs.
use rstest::rstest;

use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// An enters trigger of this permanent under `condition` doing `effects`.
fn enters(condition: &str, effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"if":{condition},"effects":[{effects}]}}}}"#
    )
}

const DRAIN: &str = r#"{"LoseLife":{"who":"Opponents","amount":3}},{"GainLife":{"amount":3}}"#;
const DRAIN_TEXT: &str = "When this creature enters, if it was bargained, each opponent loses 3 life and you gain 3 life.";

#[rstest]
#[case::bargained(enters(r#""AdditionalCostPaid""#, DRAIN), DRAIN_TEXT, true)]
#[case::kicked_is_not_bargained(enters(r#""Kicked""#, DRAIN), DRAIN_TEXT, false)]
#[case::not_bargained(enters(r#"{"Not":"AdditionalCostPaid"}"#, DRAIN), DRAIN_TEXT, false)]
#[case::other_amount(
    enters(r#""AdditionalCostPaid""#, DRAIN),
    "When this creature enters, if it was bargained, each opponent loses 2 life and you gain 3 life.",
    false
)]
#[case::dies_is_not_enters(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"if":"AdditionalCostPaid","effects":[{"LoseLife":{"who":"Opponents","amount":3}},{"GainLife":{"amount":3}}]}}"#.to_owned(),
    DRAIN_TEXT,
    false
)]
#[case::another_creature_entering(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other"],"to":"Battlefield"}},"if":"AdditionalCostPaid","effects":[{"LoseLife":{"who":"Opponents","amount":3}},{"GainLife":{"amount":3}}]}}"#.to_owned(),
    DRAIN_TEXT,
    false
)]
fn rmt4_bargained_enters_trigger(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

/// An untap limit as a static rule, under `condition` when one is given.
fn untap_limit(who: &str, what: &str, count: u32, condition: Option<&str>) -> String {
    let condition = condition.map_or_else(String::new, |c| format!(r#","condition":{c}"#));
    format!(
        r#"{{"Rule":{{"rule":{{"UntapLimit":{{"who":"{who}","what":{what},"count":{count}}}}}{condition}}}}}"#
    )
}

const UNTAPPED: &str = r#"{"Matches":{"subject":"This","selector":["Untapped"]}}"#;
const LANDS: &str = "Players can't untap more than one land during their untap steps.";

#[rstest]
#[case::players_one_land(
    untap_limit("EachPlayer", r#"[{"Type":"Land"}]"#, 1, None),
    LANDS,
    true
)]
#[case::two_lands_is_not_one(
    untap_limit("EachPlayer", r#"[{"Type":"Land"}]"#, 2, None),
    LANDS,
    false
)]
#[case::you_are_not_players(untap_limit("You", r#"[{"Type":"Land"}]"#, 1, None), LANDS, false)]
#[case::artifacts_are_not_lands(
    untap_limit("EachPlayer", r#"[{"Type":"Artifact"}]"#, 1, None),
    LANDS,
    false
)]
#[case::you_one_land(
    untap_limit("You", r#"[{"Type":"Land"}]"#, 1, None),
    "You can't untap more than one land during your untap step.",
    true
)]
#[case::two_permanents(
    untap_limit("EachPlayer", "[]", 2, Some(UNTAPPED)),
    "As long as this artifact is untapped, players can't untap more than two permanents during their untap steps.",
    true
)]
#[case::the_condition_is_said(
    untap_limit("EachPlayer", r#"[{"Type":"Land"}]"#, 1, Some(UNTAPPED)),
    LANDS,
    false
)]
#[case::nonbasic_lands(
    untap_limit(
        "EachPlayer",
        r#"[{"Not":{"Supertype":"Basic"}},{"Type":"Land"}]"#,
        1,
        None
    ),
    "Players can't untap more than one nonbasic land during their untap steps.",
    true
)]
fn rmt4_untap_limit_reads_back(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

#[rstest]
#[case::players(LANDS, untap_limit("EachPlayer", r#"[{"Type":"Land"}]"#, 1, None))]
#[case::you(
    "You can't untap more than one land during your untap step.",
    untap_limit("You", r#"[{"Type":"Land"}]"#, 1, None)
)]
#[case::two_creatures(
    "Players can't untap more than two creatures during their untap steps.",
    untap_limit("EachPlayer", r#"[{"Type":"Creature"}]"#, 2, None)
)]
fn rmt4_untap_limit_is_read(#[case] text: &str, #[case] json: String) {
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::no_number("Players can't untap more than lands during their untap steps.")]
#[case::another_step("Players can't untap more than one land during their upkeeps.")]
fn rmt4_untap_limit_near_misses_stay_unread(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Static, false)
            .is_none_or(|read| !matches!(read, Ability::Rule { .. })),
        "{text}"
    );
}

/// An attack trigger making a Boar for each player `over` names, tapped as `tapped`, attacking the player bound as `against`.
fn boars(over: &str, tapped: bool, against: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"effects":[{{"RepeatEach":{{"over":"{over}","as":"opponent","effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Boar","colors":["Green"],"types":["Creature"],"subtypes":["Boar"],"power":2,"toughness":2}}}},"tapped":{tapped},"attacking":true,"against":{{"Bound":"{against}"}}}}}}]}}}}]}}}}"#
    )
}

const BOARS: &str = "Whenever this creature attacks, for each opponent, you create a 2/2 green Boar creature token that's tapped and attacking that opponent.";

#[rstest]
#[case::each_opponent(boars("Opponents", true, "opponent"), BOARS, true)]
#[case::that_player(
    boars("Opponents", true, "opponent"),
    "Whenever this creature attacks, for each opponent, create a 2/2 green Boar creature token that's tapped and attacking that player.",
    true
)]
#[case::each_player_is_not_each_opponent(boars("EachPlayer", true, "opponent"), BOARS, false)]
#[case::untapped_is_not_tapped(boars("Opponents", false, "opponent"), BOARS, false)]
#[case::another_name_is_not_that_opponent(boars("Opponents", true, "someone"), BOARS, false)]
fn rmt4_a_token_attacking_each_opponent(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

/// A static over the other creatures you control of the subtypes, with the modifications.
fn group_static(subtypes: &[&str], mods: &str) -> String {
    let any: Vec<String> = subtypes
        .iter()
        .map(|s| format!(r#"{{"Subtype":"{s}"}}"#))
        .collect();
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},"Other",{{"ControlledBy":"You"}},{{"AnyOf":[{}]}}]}},"mods":[{mods}]}}}}"#,
        any.join(",")
    )
}

const PUMP: &str = r#"{"ModifyPT":{"power":1,"toughness":1}}"#;
const PUMP_VIGILANCE: &str = r#"{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Vigilance"}"#;
const FIVE: [&str; 5] = ["Cat", "Elemental", "Nightmare", "Dinosaur", "Beast"];

#[rstest]
#[case::bare_list(
    group_static(&FIVE, PUMP_VIGILANCE),
    "Each other creature you control that's a Cat, Elemental, Nightmare, Dinosaur, or Beast gets +1/+1 and has vigilance.",
    true
)]
#[case::a_missing_subtype(
    group_static(&FIVE[..4], PUMP_VIGILANCE),
    "Each other creature you control that's a Cat, Elemental, Nightmare, Dinosaur, or Beast gets +1/+1 and has vigilance.",
    false
)]
#[case::other_two_subtypes(
    group_static(&["Ninja", "Rogue"], PUMP),
    "Other Ninja and Rogue creatures you control get +1/+1.",
    true
)]
#[case::two_subtypes_with_a_keyword(
    group_static(&["Ninja", "Rogue"], PUMP_VIGILANCE),
    "Other Ninja and Rogue creatures you control get +1/+1 and have vigilance.",
    true
)]
#[case::another_subtype(group_static(&["Ninja", "Pirate"], PUMP), "Other Ninja and Rogue creatures you control get +1/+1.", false)]
#[case::three_subtypes_are_not_two(
    group_static(&["Ninja", "Rogue", "Pirate"], PUMP),
    "Other Ninja and Rogue creatures you control get +1/+1.",
    false
)]
fn rmt4_creatures_of_several_subtypes(
    #[case] json: String,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&json, clause), reads, "{json}");
}

/// Haste and ward {10} for this permanent while `selector` holds for it.
fn while_it(selector: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"AddKeyword":"Haste"}},{{"AddKeyword":{{"Ward":"{{10}}"}}}}],"condition":{{"Matches":{{"subject":"This","selector":[{selector}]}}}}}}}}"#
    )
}

const TARRASQUE: &str = "~ has haste and ward {10} as long as it was cast.";

#[rstest]
#[case::cast_from_anywhere(while_it(r#"{"WasCast":{}}"#), TARRASQUE, true)]
#[case::cast_from_the_hand_only(while_it(r#"{"WasCast":{"from":"Hand"}}"#), TARRASQUE, false)]
#[case::cast_by_you_only(while_it(r#"{"WasCast":{"by":"You"}}"#), TARRASQUE, false)]
#[case::tapped_is_not_cast(while_it(r#""Tapped""#), TARRASQUE, false)]
fn rmt4_as_long_as_it_was_cast(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}
