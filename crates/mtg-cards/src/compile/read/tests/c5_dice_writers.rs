//! Goal: the writers of the (r1at) pass give the oracle wording of two piles of the top cards of a library, as a spell and after a trigger and for any number of cards, and piles whose opponent, split, choice or moves differ never read back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// The effects of the piles of the top `count` cards, with the separator, the split and the moves as given.
fn piles(count: u32, separator: &str, split: &str, kept: &str, left: &str) -> String {
    format!(
        r#"[{{"LookAtTop":{{"count":{count},"as":"top"}}}},{{"Reveal":{{"what":{{"Bound":"top"}}}}}},{{"Choose":{{"from":"Opponents","as":"separator"}}}},{{"Choose":{{"who":{separator},"from":{{"Each":[{{"In":"Library"}},{{"Is":{{"Bound":"top"}}}}]}},"count":{split},"as":"pile","rest_as":"other"}}}},{{"May":{{"effects":[{{"Move":{{"what":{{"Bound":"pile"}},"to":"Hand"}}}},{{"Move":{{"what":{{"Bound":"other"}},"to":"Graveyard"}}}}]}}}},{{"If":{{"condition":{{"Not":"Performed"}},"then":[{{"Move":{{"what":{{"Bound":"{kept}"}},"to":"Hand"}}}},{{"Move":{{"what":{{"Bound":"{left}"}},"to":"Graveyard"}}}}]}}}}]"#
    )
}

const SEPARATOR: &str = r#"{"Bound":"separator"}"#;
const ANY: &str = r#"{"min":0,"max":null}"#;

fn spell(effects: &str) -> String {
    format!(r#"{{"Spell":{{"effects":{effects}}}}}"#)
}

fn enters(effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":{effects}}}}}"#
    )
}

const FACT_OR_FICTION: &str = "Reveal the top five cards of your library. An opponent separates those cards into two piles. Put one pile into your hand and the other into your graveyard.";

#[rstest]
#[case::a_spell(spell(&piles(5, SEPARATOR, ANY, "other", "pile")), FACT_OR_FICTION)]
#[case::an_enters_trigger(
    enters(&piles(5, SEPARATOR, ANY, "other", "pile")),
    "When this creature enters, reveal the top five cards of your library. An opponent separates those cards into two piles. Put one pile into your hand and the other into your graveyard."
)]
#[case::three_cards(
    spell(&piles(3, SEPARATOR, ANY, "other", "pile")),
    "Reveal the top three cards of your library. An opponent separates those cards into two piles. Put one pile into your hand and the other into your graveyard."
)]
fn two_piles_read(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::each_opponent_splits(spell(&piles(5, r#""Opponents""#, ANY, "other", "pile")))]
#[case::you_split(spell(&piles(5, r#""You""#, ANY, "other", "pile")))]
#[case::one_card_in_a_pile(spell(&piles(5, SEPARATOR, r#"{"min":1,"max":1}"#, "other", "pile")))]
#[case::the_same_pile_twice(spell(&piles(5, SEPARATOR, ANY, "pile", "other")))]
fn piles_that_differ_do_not_read(#[case] json: String) {
    assert!(!written(&json, FACT_OR_FICTION), "{json}");
}
