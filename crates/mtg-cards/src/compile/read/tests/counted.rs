//! Goal: the costs that count the X the player announces ("Pay X life", "Remove X ki counters from ~", "Sacrifice X creatures", "Discard X cards", "Exile X cards from your graveyard", "Tap X untapped artifacts you control", "Pay X {E}") and the energy of a cost and an effect ("Pay {E}{E}", "you get {E}{E}") read into the DSL in the shapes of the hand-written files, write back as the same text, set the X of the effects after the colon, and leave near-misses (an X in a trigger, a count the engine cannot pay) to the table.
use super::*;

#[rstest]
#[case::life(
    "Pay X life: Draw X cards.",
    r#"{"Activated":{"cost":[{"PayLife":"X"}],"effects":[{"Draw":{"count":"X"}}]}}"#
)]
#[case::counters(
    "{1}, {T}, Remove X ki counters from ~: Draw X cards.",
    r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap",{"RemoveCounters":{"counter":{"Other":"ki"},"count":"X"}}],"effects":[{"Draw":{"count":"X"}}]}}"#
)]
#[case::sacrifice(
    "{T}, Sacrifice X lands: Put X +1/+1 counters on ~.",
    r#"{"Activated":{"cost":["Tap",{"Sacrifice":{"filter":[{"Type":"Land"}],"count":"X"}}],"effects":[{"PutCounters":{"on":"This","counter":"+1/+1","count":"X"}}]}}"#
)]
#[case::discard(
    "Discard X cards: Draw X cards.",
    r#"{"Activated":{"cost":[{"Discard":{"count":"X"}}],"effects":[{"Draw":{"count":"X"}}]}}"#
)]
#[case::exile(
    "{B}, Exile X cards from your graveyard: You gain X life.",
    r#"{"Activated":{"cost":[{"Mana":"{B}"},{"ExileFromGraveyard":{"filter":[],"count":"X"}}],"effects":[{"GainLife":{"amount":"X"}}]}}"#
)]
#[case::tap(
    "{2}, {T}, Tap X untapped artifacts you control: Draw X cards.",
    r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap",{"TapPermanents":{"filter":[{"Type":"Artifact"}],"count":"X"}}],"effects":[{"Draw":{"count":"X"}}]}}"#
)]
#[case::energy_x(
    "{T}, Pay X {E}: Draw X cards.",
    r#"{"Activated":{"cost":["Tap",{"PayEnergy":"X"}],"effects":[{"Draw":{"count":"X"}}]}}"#
)]
fn a_cost_that_counts_the_announced_x_reads_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::paying(
    "{T}, Pay {E}: Tap target creature.",
    r#"{"Activated":{"cost":["Tap",{"PayEnergy":1}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#
)]
#[case::paying_two(
    "Pay {E}{E}: Draw a card.",
    r#"{"Activated":{"cost":[{"PayEnergy":2}],"effects":[{"Draw":{}}]}}"#
)]
fn energy_is_paid_as_a_cost(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::two(
    "When ~ enters, you get {E}{E}.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":2}}]}}"#
)]
#[case::in_a_sentence_with_others(
    "When ~ enters, draw a card. You get {E}.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{}},{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":1}}]}}"#
)]
fn energy_is_gained_as_an_effect(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[test]
fn the_reminder_text_of_energy_counters_does_not_keep_the_clause_from_being_read() {
    use crate::compile::normalize::reminder_mentions_counter;
    assert!(!reminder_mentions_counter(
        "you get {E}{E} (two energy counters)."
    ));
    assert!(reminder_mentions_counter(
        "put a shield counter (a shield counter prevents damage)."
    ));
    assert!(!reminder_mentions_counter(
        "Create two 1/1 green Phyrexian Insect creature tokens with infect. (They deal damage to creatures in the form of -1/-1 counters and to players in the form of poison counters.)"
    ));
    assert!(!reminder_mentions_counter(
        "Time travel. (For each suspended card you own and each permanent you control with a time counter on it, you may add or remove a time counter.)"
    ));
    assert!(!reminder_mentions_counter(
        "Incubate 3. (Create an Incubator token with three +1/+1 counters on it and \"{2}: Transform this token.\" It transforms into a 0/0 Phyrexian artifact creature.)"
    ));
}

#[rstest]
#[case::x_in_a_trigger("When ~ enters, you get X {E}.", ClauseKind::Triggered)]
#[case::a_count_the_engine_cannot_pay(
    "Pay life equal to its power: Draw a card.",
    ClauseKind::Activated
)]
#[case::x_without_what_it_counts("Sacrifice X: Draw a card.", ClauseKind::Activated)]
fn what_the_templates_do_not_read_is_left_to_the_table(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
