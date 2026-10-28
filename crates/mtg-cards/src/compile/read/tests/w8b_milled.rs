//! Goal: a mill trigger ("whenever a player mills a nonland card", "an opponent mills", "one or more nonland cards are milled") reads into `Milled` (a batch for "one or more") and writes back the same way, and a move into a graveyard that is no mill stays with the other patterns; ", then" and "you get counters and put" join two steps, "if this creature is in your graveyard" makes a return work from there, and "you gain life rather than lose life from radiation" is a rule.
use super::*;

#[rstest]
#[case::a_player_mills(
    "Whenever a player mills a nonland card, you gain 1 life.",
    r#"{"Triggered":{"when":{"Milled":{"what":[{"Not":{"Type":"Land"}}]}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::an_opponent_mills(
    "Whenever an opponent mills a nonland card, you gain 1 life.",
    r#"{"Triggered":{"when":{"Milled":{"what":[{"Not":{"Type":"Land"}}],"by":"Opponents"}},"effects":[{"GainLife":{"amount":1}}]}}"#
)]
#[case::you_mill_a_creature_card(
    "Whenever you mill a creature card, draw a card.",
    r#"{"Triggered":{"when":{"Milled":{"what":[{"Type":"Creature"}],"by":"You"}},"effects":[{"Draw":{}}]}}"#
)]
#[case::one_or_more(
    "Whenever one or more nonland cards are milled, draw a card.",
    r#"{"Triggered":{"when":{"Batch":{"Milled":{"what":[{"Not":{"Type":"Land"}}]}}},"effects":[{"Draw":{}}]}}"#
)]
fn w8b_a_mill_trigger(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::then_once_each_turn(
    "Whenever one or more nonland cards are milled, draw a card, then put a +1/+1 counter on this creature. This ability triggers only once each turn.",
    r#"{"Triggered":{"when":{"Batch":{"Milled":{"what":[{"Not":{"Type":"Land"}}]}}},"limit":"OncePerTurn","effects":[{"Draw":{}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::then_after_another_event(
    "Whenever a creature dies, draw a card, then put a +1/+1 counter on this creature.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#
)]
#[case::in_your_graveyard(
    "Whenever an opponent mills a nonland card, if this creature is in your graveyard, you may return it to your hand.",
    r#"{"Triggered":{"when":{"Milled":{"what":[{"Not":{"Type":"Land"}}],"by":"Opponents"}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#
)]
#[case::in_your_graveyard_after_a_death(
    "Whenever a creature dies, if this creature is in your graveyard, you may return it to your hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#
)]
#[case::you_get_counters_and_put(
    "Whenever this creature is dealt damage, you get three rad counters and put three +1/+1 counters on this creature.",
    r#"{"Triggered":{"when":{"DealsDamage":{"source":[],"to":"This"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"rad"},"count":3}},{"PutCounters":{"on":"This","counter":"+1/+1","count":3}}]}}"#
)]
fn w8b_a_whole_ability(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[test]
fn w8b_radiation_gains_life_reads_as_a_rule() {
    reads_as(
        "You gain life rather than lose life from radiation.",
        ClauseKind::Static,
        false,
        r#"{"Rule":{"rule":{"RadiationGainsLife":"You"}}}"#,
    );
}

#[rstest]
#[case::then_an_unlisted_verb("Whenever a creature dies, draw a card, then gain 1 life.")]
#[case::you_gain_life_and_put(
    "Whenever a creature dies, you gain 3 life and put a +1/+1 counter on this creature."
)]
#[case::an_opponent_gets_counters_and_put(
    "Whenever a creature dies, each opponent gets a rad counter and put a +1/+1 counter on this creature."
)]
#[case::in_your_graveyard_then_another_effect(
    "Whenever a creature dies, if this creature is in your graveyard, you may draw a card."
)]
#[case::in_another_graveyard(
    "Whenever a creature dies, if this creature is in a graveyard, you may return it to your hand."
)]
fn w8b_other_wholes_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

#[test]
fn w8b_then_joins_each_imperative_sentence_once() {
    let texts = ["Draw a card. Put a +1/+1 counter on ~. Create a Food token.".to_owned()];
    let joined = crate::compile::read::w8b_then::w8b_then_wordings(&texts);
    assert_eq!(
        joined,
        [
            "Draw a card, then put a +1/+1 counter on ~. Create a Food token.",
            "Draw a card. Put a +1/+1 counter on ~, then create a Food token.",
        ]
    );
}

#[rstest]
#[case::a_creature_you_control("Whenever a player mills a creature you control, you gain 1 life.")]
#[case::another_card("Whenever a player mills another card, you gain 1 life.")]
#[case::one_or_more_put("Whenever one or more nonland cards are put, draw a card.")]
fn w8b_other_mills_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
