//! Goal: a token-creation replacement multiplies the tokens of the player it names and of the kind it names, and a form
//! the node cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn card(line: &str, part: &str) -> String {
    format!("Name:T\nTypes:Enchantment\nR:Event$ CreateToken | {line}\nSVar:Rep:{part}\n")
}

#[rstest]
#[case::double("ActiveZones$ Battlefield | ValidToken$ Card.YouCtrl | ReplaceWith$ Rep | EffectOnly$ True", "DB$ ReplaceToken | Type$ Amount", json!({"controller": "You"}), 2)]
#[case::triple("ActiveZones$ Battlefield | ValidToken$ Card.OppCtrl | ReplaceWith$ Rep", "DB$ ReplaceToken | Type$ Amount | Amount$ Thrice", json!({"controller": "Opponents"}), 3)]
#[case::creatures("ActiveZones$ Battlefield | ValidToken$ Creature.YouCtrl | ReplaceWith$ Rep", "DB$ ReplaceToken | Type$ Amount", json!({"what": [{"Type": "Creature"}]}), 2)]
fn a_token_replacement_multiplies(
    #[case] line: &str,
    #[case] part: &str,
    #[case] when: Value,
    #[case] by: u32,
) {
    let script = card(line, part);
    let mut found = abilities(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    let want = json!({"Replacement": {"when": {"TokenCreation": when}, "by": {"Multiply": by}}});
    assert_eq!(normal(found.remove(0)), normal(want));
}

#[rstest]
#[case::halve(
    "ActiveZones$ Battlefield | ValidToken$ Card.OppCtrl | ReplaceWith$ Rep",
    "DB$ ReplaceToken | Type$ Amount | Amount$ Half"
)]
#[case::add_token(
    "ActiveZones$ Battlefield | ValidToken$ Card.YouCtrl | ReplaceWith$ Rep",
    "DB$ ReplaceToken | Type$ AddToken | Amount$ 1 | TokenScript$ c_a_treasure_sac"
)]
#[case::optional(
    "ActiveZones$ Battlefield | ValidToken$ Card.YouCtrl | ReplaceWith$ Rep | Optional$ True",
    "DB$ ReplaceToken | Type$ Amount"
)]
#[case::command_zone(
    "ActiveZones$ Command | ValidToken$ Card.YouCtrl | ReplaceWith$ Rep",
    "DB$ ReplaceToken | Type$ Amount"
)]
fn a_form_beyond_the_node_stays_a_gap(#[case] line: &str, #[case] part: &str) {
    let script = card(line, part);
    assert_eq!(abilities(&script), Vec::<Value>::new(), "{script}");
    assert_ne!(gaps(&script), Vec::<String>::new());
}
