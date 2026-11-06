//! Goal: the count words of spent mana and a promised gift, and the cost words that name a card's own mana value or an alternative cost, translate to the engine's amounts and conditions.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

#[rstest]
#[case::escape("Escape:CardManaCost ExileFromGrave<3/Card.Other/other>", Some(json!({"Escape": ["OwnManaCost", {"ExileFromGraveyard": {"count": 3, "filter": ["Other"]}}]})))]
#[case::scavenge("Scavenge:CardManaCost", Some(json!({"Scavenge": ["OwnManaCost"]})))]
#[case::encore("Encore:CardManaCost", Some(json!({"Encore": ["OwnManaCost"]})))]
// Read from the printed card at the time of the cast, which a given keyword is not.
#[case::replicate("Replicate:CardManaCost", None)]
#[case::emerge("Emerge:CardManaCost", None)]
fn a_keyword_given_with_the_cards_own_mana_cost_is_read_off_each_card(
    #[case] keyword: &str,
    #[case] expected: Option<Value>,
) {
    let text = format!(
        "Name:T\nTypes:Enchantment\nS:Mode$ Continuous | Affected$ Card.YouOwn | AffectedZone$ Graveyard | AddKeyword$ {keyword}\n"
    );
    match expected {
        Some(keyword) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            assert_eq!(
                abilities(&text)[0]["Static"]["mods"],
                json!([{"AddKeyword": keyword}])
            );
        }
        None => assert_eq!(gaps(&text), vec!["cost:CardManaCost".to_owned()]),
    }
}

#[test]
fn the_cards_own_mana_cost_is_no_cost_word_of_an_ability() {
    let text =
        "Name:T\nTypes:Creature Elf\nPT:1/1\nA:AB$ Draw | Cost$ CardManaCost | NumCards$ 1\n";
    assert_eq!(gaps(text), vec!["cost:CardManaCost".to_owned()]);
}

#[rstest]
#[case::all_mana("Count$CastTotalManaSpent", json!({"ManaSpent": {"of": null}}))]
fn the_mana_spent_is_counted_by_the_kind_of_its_source(#[case] x: &str, #[case] expected: Value) {
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:{x}\n");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text)[0]["Spell"]["effects"][0]["Draw"]["count"],
        expected
    );
}

#[rstest]
#[case::permanent_beside_others("Permanent.Snow,Legendary,Saga", false)]
#[case::only_permanents("Permanent.Creature+cmcLE3", true)]
fn a_search_for_a_permanent_beside_other_alternatives_is_refused(
    #[case] spec: &str,
    #[case] read: bool,
) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChangeZone | Origin$ Library | Destination$ Hand | ChangeType$ {spec} | ChangeNum$ 1\n"
    );
    let found = gaps(&text);
    if read {
        assert_eq!(found, Vec::<String>::new());
    } else {
        assert_eq!(
            found,
            vec!["valid:Permanent-card-beside-other-alternatives".to_owned()]
        );
    }
}

#[rstest]
#[case::unknown_subtype("Count$CastTotalManaSpent Wibble")]
#[case::two_words("Count$CastTotalManaSpent Snow Treasure")]
fn a_source_the_reader_does_not_know_stays_a_gap(#[case] x: &str) {
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:{x}\n");
    assert_eq!(
        gaps(&text),
        vec!["amount:Count$CastTotalManaSpent-source".to_owned()]
    );
}

#[rstest]
#[case::promised_gift_amount("Count$PromisedGift.2.1", json!({"When": {
    "condition": {"Compare": {"left": "Promised", "op": "Ge", "right": 1}}, "then": 2, "otherwise": 1}}))]
fn a_count_of_the_promised_gift_is_a_choice_between_two_amounts(
    #[case] x: &str,
    #[case] expected: Value,
) {
    let text = format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ X\nSVar:X:{x}\n");
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text)[0]["Spell"]["effects"][0]["Draw"]["count"],
        expected
    );
}
