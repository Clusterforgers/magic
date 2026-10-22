//! Goal: the triggers and costs of the mana permanents that count (Mana Bloom, Ramos, Cadaverous Bloom) read into the DSL and write back in the same words: an upkeep trigger on no counters that returns the card, a counter for each color of the spell cast, a mana ability that removes counters or exiles a card from the hand; near-misses stay with the table.
use super::*;

#[rstest]
#[case::no_counters_returns_the_card(
    "At the beginning of your upkeep, if ~ has no charge counters on it, return it to its owner's hand.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Counters":{"on":"This","counter":"charge"}},"op":"Eq","right":0}},"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#
)]
#[case::another_counter(
    "At the beginning of your upkeep, if ~ has no +1/+1 counters on it, return it to its owner's hand.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Eq","right":0}},"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#
)]
#[case::a_counter_for_each_color(
    "Whenever you cast a spell, put a +1/+1 counter on ~ for each of that spell's colors.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Stat":{"of":"Triggering","stat":"Colors"}}}}]}}"#
)]
#[case::several_counters_removed(
    "Remove five +1/+1 counters from ~: Add {W}{W}{U}{U}{B}{B}{R}{R}{G}{G}. Activate only once each turn.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"+1/+1","count":5}}],"limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"Fixed":["White","White","Blue","Blue","Black","Black","Red","Red","Green","Green"]}}}]}}"#
)]
#[case::a_card_exiled_from_the_hand(
    "Exile a card from your hand: Add {B}{B} or {G}{G}.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":[{"ExileFromHand":{"filter":[]}}],"effects":[{"AddMana":{"mana":{"OneOfLists":[["Black","Black"],["Green","Green"]]}}}]}}"#
)]
fn the_wordings_of_the_counting_mana_permanents_read_and_write_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::the_counters_of_another_card(
    "At the beginning of your upkeep, if target creature has no charge counters on it, return it to its owner's hand."
)]
#[case::a_number_of_counters(
    "At the beginning of your upkeep, if ~ has two charge counters on it, return it to its owner's hand."
)]
#[case::it_before_the_card_is_named(
    "At the beginning of your upkeep, return it to its owner's hand."
)]
#[case::the_colors_of_another_thing(
    "Whenever you cast a spell, put a +1/+1 counter on ~ for each of its colors."
)]
fn near_misses_of_the_counting_mana_permanents_stay_with_the_table(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
