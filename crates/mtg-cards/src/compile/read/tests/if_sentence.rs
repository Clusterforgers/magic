//! Goal: "If there are five or more hatchling counters on ~, remove all of them and transform ~." (also "Then if ... on it", "or fewer") reads as an `Effect::If` on a count of the counters on the card, writes back in each wording ("Then if", "it" for the card), and a condition, step or place the template does not model stays with the table.
use super::*;

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

const LUDEVIC: &str = r#"{"Activated":{"cost":"{1}{U}","effects":[{"PutCounters":{"on":"This","counter":{"Other":"hatchling"}}},{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"hatchling"}}},"op":"Ge","right":5}},"then":[{"RemoveCounters":{"from":"This","counter":{"Other":"hatchling"},"count":{"Counters":{"on":"This","counter":{"Other":"hatchling"}}}}},{"Transform":{"what":"This"}}]}}]}}"#;

#[rstest]
#[case::then_if_and_it(
    "{1}{U}: Put a hatchling counter on this creature. Then if there are five or more hatchling counters on it, remove all of them and transform it."
)]
#[case::if_and_the_card(
    "{1}{U}: Put a hatchling counter on ~. If there are five or more hatchling counters on ~, remove all of them and transform ~."
)]
#[case::then_if_and_the_card(
    "{1}{U}: Put a hatchling counter on ~. Then if there are five or more hatchling counters on ~, remove all of them and transform ~."
)]
#[case::the_card_then_it(
    "{1}{U}: Put a hatchling counter on ~. If there are five or more hatchling counters on ~, remove all of them and transform it."
)]
fn the_count_of_counters_reads_in_every_wording_of_ludevics_test_subject(#[case] text: &str) {
    reads_as(text, ClauseKind::Activated, false, LUDEVIC);
}

#[rstest]
#[case::at_least(
    "Whenever ~ attacks, put a charge counter on ~. If there are three or more charge counters on it, transform ~.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"PutCounters":{"on":"This","counter":"charge"}},{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":"charge"}},"op":"Ge","right":3}},"then":[{"Transform":{"what":"This"}}]}}]}}"#
)]
#[case::at_most(
    "Whenever ~ attacks, if there are two or fewer time counters on ~, draw a card.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"time"}}},"op":"Le","right":2}},"then":[{"Draw":{}}]}}]}}"#
)]
#[case::a_plus_one_counter(
    "Whenever ~ attacks, if there are four or more +1/+1 counters on ~, you gain 3 life.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":4}},"then":[{"GainLife":{"amount":3}}]}}]}}"#
)]
#[case::steps_in_a_list(
    "Whenever ~ attacks, if there are five or more charge counters on ~, remove all of them, draw a card, and transform ~.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":"charge"}},"op":"Ge","right":5}},"then":[{"RemoveCounters":{"from":"This","counter":"charge","count":{"Counters":{"on":"This","counter":"charge"}}}},{"Draw":{}},{"Transform":{"what":"This"}}]}}]}}"#
)]
fn the_count_of_counters_condition_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::one_counter(
    "Whenever ~ attacks, if there are one or more charge counters on ~, draw a card."
)]
#[case::a_number_other_than_a_word(
    "Whenever ~ attacks, if there are 5 or more charge counters on ~, draw a card."
)]
#[case::exactly("Whenever ~ attacks, if there are exactly five charge counters on ~, draw a card.")]
#[case::more_than(
    "Whenever ~ attacks, if there are more than five charge counters on ~, draw a card."
)]
#[case::another_permanent(
    "Whenever ~ attacks, if there are five or more charge counters on target creature, draw a card."
)]
#[case::any_counters("Whenever ~ attacks, if there are five or more counters on ~, draw a card.")]
#[case::a_counter_the_rules_give(
    "Whenever ~ attacks, if there are five or more stun counters on ~, draw a card."
)]
#[case::no_comma("Whenever ~ attacks, if there are five or more charge counters on ~ draw a card.")]
#[case::an_unread_step(
    "Whenever ~ attacks, if there are five or more charge counters on ~, flip a coin."
)]
#[case::remove_all_of_them_without_a_count("Whenever ~ attacks, remove all of them.")]
#[case::an_otherwise(
    "Whenever ~ attacks, if there are five or more charge counters on ~, draw a card. Otherwise, gain 1 life."
)]
#[case::an_unread_condition(
    "Whenever ~ attacks, if an opponent controls more lands than you, draw a card."
)]
#[case::then_first(
    "Whenever ~ attacks, then if there are five or more charge counters on ~, draw a card."
)]
fn a_condition_the_template_does_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::it_after_a_step_that_is_not_the_card(
    "{1}{U}: Tap target creature. Then if there are five or more hatchling counters on it, transform ~."
)]
#[case::remove_all_of_a_kind_not_counted(
    "{1}{U}: Put a hatchling counter on ~. Then if there are five or more hatchling counters on it, remove all of them from target creature."
)]
fn the_card_is_the_only_place_the_counters_are_counted(#[case] text: &str) {
    assert_eq!(activated(text), None, "{text}");
}

#[rstest]
fn every_count_kind_and_wording_writes_back_as_it_was_read(
    #[values("two", "five", "twenty")] number: &str,
    #[values("or more", "or fewer")] bound: &str,
    #[values("charge", "time", "+1/+1", "-1/-1", "hatchling", "poison")] kind: &str,
    #[values("~", "it")] pronoun: &str,
    #[values("If", "Then if")] opener: &str,
) {
    let text = format!(
        "Whenever ~ attacks, put a {kind} counter on ~. {opener} there are {number} {bound} {kind} counters on {pronoun}, remove all of them and transform {pronoun}."
    );
    golden(&text, ClauseKind::Triggered, false);
}
