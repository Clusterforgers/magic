//! Goal: pack e19's Forge lines read: an `Event$ Mill` replacement of a player's mill as twice that many or that many
//! plus a number (another change of the mill, a condition or another player stays a gap), and a return whose static
//! names creature types with `RemoveCreatureTypes$` as those types replacing the others (a word that is no creature type
//! is a gap).
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// A permanent with one mill replacement whose `ReplaceEffect` sets the number to `x`.
fn mill(valid: &str, extra: &str, x: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nR:Event$ Mill | ActiveZones$ Battlefield | ValidPlayer$ {valid}{extra} | ReplaceWith$ More | Description$ x\nSVar:More:DB$ ReplaceEffect | VarName$ Number | VarValue$ X\nSVar:X:{x}\n"
    )
}

#[rstest]
#[case::twice("Player.Opponent", "ReplaceCount$Number/Twice", json!({"who": "Opponents"}), json!({"Multiply": 2}))]
#[case::plus_four("Player.Opponent", "ReplaceCount$Number/Plus.4", json!({"who": "Opponents"}), json!({"Add": 4}))]
#[case::you("You", "ReplaceCount$Number/Twice", json!({}), json!({"Multiply": 2}))]
fn a_mill_replacement_reads(
    #[case] valid: &str,
    #[case] x: &str,
    #[case] who: Value,
    #[case] by: Value,
) {
    let text = mill(valid, "", x);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text)[0],
        json!({"Replacement": {"when": {"Mill": who}, "by": by}})
    );
}

#[rstest]
#[case::another_change("Player.Opponent", "", "ReplaceCount$Number/HalfUp")]
#[case::condition("Player.Opponent", " | CheckSVar$ Y", "ReplaceCount$Number/Twice")]
#[case::another_player("Player.Other+withFlying", "", "ReplaceCount$Number/Twice")]
fn another_mill_replacement_is_a_gap(#[case] valid: &str, #[case] extra: &str, #[case] x: &str) {
    assert_ne!(gaps(&mill(valid, extra, x)), Vec::<String>::new());
}

/// "{T}: Put target creature card in a graveyard onto the battlefield under your control" with the static `animate`.
fn returned(animate: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ ChangeZone | Cost$ T | Origin$ Graveyard | Destination$ Battlefield | GainControl$ True | ValidTgts$ Creature | StaticEffect$ Animate | SpellDescription$ x\nSVar:Animate:Mode$ Continuous | Affected$ Card.IsRemembered | {animate}\n"
    )
}

#[rstest]
#[case::green_mutant(
    "AddType$ Mutant | SetColor$ Green | SetPower$ 3 | SetToughness$ 3 | RemoveCreatureTypes$ True",
    json!([{"SetPT": {"power": 3, "toughness": 3}}, {"SetCreatureTypes": ["Mutant"]}, {"SetColors": ["Green"]}])
)]
#[case::spirit_detective(
    "AddType$ Spirit & Detective | RemoveCreatureTypes$ True",
    json!([{"SetCreatureTypes": ["Spirit", "Detective"]}])
)]
#[case::with_a_card_type(
    "AddType$ Artifact & Golem | RemoveCreatureTypes$ True",
    json!([{"AddType": "Artifact"}, {"SetCreatureTypes": ["Golem"]}])
)]
fn the_creature_types_a_return_names_replace_the_others(
    #[case] animate: &str,
    #[case] mods: Value,
) {
    let text = returned(animate);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let effects = &abilities(&text)[0]["Activated"]["effects"];
    assert_eq!(effects[1]["Continuous"]["mods"], mods);
    assert_eq!(effects[1]["Continuous"]["duration"], json!("Forever"));
}

#[rstest]
#[case::no_creature_type("SetColor$ Green | RemoveCreatureTypes$ True")]
#[case::a_land_type("AddType$ Forest & Elf | RemoveCreatureTypes$ True")]
#[case::an_unknown_word("AddType$ Blorp | RemoveCreatureTypes$ True")]
fn a_return_that_names_no_creature_types_alone_is_a_gap(#[case] animate: &str) {
    assert!(
        gaps(&returned(animate))
            .iter()
            .any(|gap| gap.contains("RemoveCreatureTypes")),
        "{animate}"
    );
}
