//! Goal: conjure and seek map to their engine nodes for the shapes the nodes say, and every other shape of the two verbs
//! stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Win:DB$ GainLife | LifeAmount$ 2\n")
}

#[rstest]
#[case::hand(
    "MakeCard | Name$ Lightning Bolt | Conjure$ True | Zone$ Hand",
    json!({"Conjure": {"name": "Lightning Bolt", "zone": "Hand"}})
)]
#[case::graveyard(
    "MakeCard | Name$ Ponder | Conjure$ True | Zone$ Graveyard | Amount$ 2",
    json!({"Conjure": {"name": "Ponder", "zone": "Graveyard", "count": 2}})
)]
#[case::exile(
    "MakeCard | Name$ Mox Opal | Conjure$ True | Zone$ Exile | Defined$ Opponent",
    json!({"Conjure": {"who": "Opponents", "name": "Mox Opal", "zone": "Exile"}})
)]
#[case::seek_creature("Seek | Type$ Card.Creature", json!({"Seek": {"filter": [{"Type": "Creature"}]}}))]
#[case::seek_two("Seek | Type$ Card.Creature | Num$ 2", json!({"Seek": {"filter": [{"Type": "Creature"}], "count": 2}}))]
#[case::seek_land("Seek | Type$ Land", json!({"Seek": {"filter": [{"Type": "Land"}]}}))]
fn a_digital_verb_maps_to_its_node(#[case] line: &str, #[case] node: Value) {
    let script = spell(line);
    let found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let want = json!({"Spell": {"effects": [node]}});
    assert_eq!(normal(found[0].clone()), normal(want));
}

#[rstest]
#[case::unstored_name(
    "MakeCard | Name$ Nobody Knows | Conjure$ True | Zone$ Hand",
    "name:MakeCard.Unstored"
)]
#[case::chosen_name(
    "MakeCard | Name$ ChosenName | Conjure$ True | Zone$ Hand",
    "key:MakeCard.Name"
)]
#[case::battlefield(
    "MakeCard | Name$ Ponder | Conjure$ True | Zone$ Battlefield",
    "zone:MakeCard.Battlefield"
)]
#[case::library(
    "MakeCard | Name$ Ponder | Conjure$ True | Zone$ Library",
    "zone:MakeCard.Library"
)]
#[case::token_card(
    "MakeCard | Name$ Ponder | TokenCard$ True | Zone$ Hand",
    "key:MakeCard.TokenCard"
)]
#[case::not_conjured("MakeCard | Name$ Ponder | Zone$ Hand", "key:MakeCard.Conjure")]
#[case::defined_name(
    "MakeCard | DefinedName$ Targeted | Conjure$ True | Zone$ Hand",
    "key:MakeCard.DefinedName"
)]
#[case::spellbook(
    "MakeCard | Spellbook$ A,B | Conjure$ True | Zone$ Hand",
    "key:MakeCard.Spellbook"
)]
#[case::remembered(
    "MakeCard | Name$ Ponder | Conjure$ True | Zone$ Hand | RememberMade$ True",
    "key:MakeCard.RememberMade"
)]
#[case::seek_remembered(
    "Seek | Type$ Card.Creature | RememberFound$ True",
    "key:Seek.RememberFound"
)]
#[case::seek_destination(
    "Seek | Type$ Card.Creature | Destination$ Battlefield",
    "key:Seek.Destination"
)]
fn a_form_beyond_the_node_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = spell(line);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}
