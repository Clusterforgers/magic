//! Goal: a Role token attached to a creature translates to a create and an attach; the one-Role-per-controller rule is the engine's state-based action.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;
use serde_json::json;

use super::r3p5::token_card;

#[rstest]
#[case::target("Targeted", "Creature.YouCtrl+Other", json!({"Target": 0}))]
fn a_role_token_is_created_and_attached(
    #[case] to: &str,
    #[case] valid: &str,
    #[case] host: serde_json::Value,
) {
    let text = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Token | TokenScript$ role_test | TokenOwner$ You | AttachedTo${to} | ValidTgts$ {valid}\n"
    );
    let card = token_card(&text);
    assert_eq!(card.gaps().count(), 0);
    let found = card.faces.into_iter().next().unwrap().abilities;
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[1]["Attach"]["to"], host);
    assert_eq!(
        effects[1]["Attach"]["what"],
        json!({"Bound": effects[0]["CreateToken"]["as"].clone()})
    );
}

use super::{abilities, gaps};

#[rstest]
#[case::generic("2", "{2}")]
#[case::colored("2 U", "{2}{U}")]
#[case::two_colors("1 G G", "{1}{G}{G}")]
fn strive_costs_more_for_each_target_beyond_the_first(#[case] cost: &str, #[case] mana: &str) {
    let text = format!(
        "Name:T\nTypes:Instant\nK:Strive:{cost}\nA:SP$ Draw | ValidTgts$ Player | SpellDescription$ Draw.\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    assert_eq!(
        found[0],
        json!({"CostModifier": {"applies_to": ["This"], "change": "Increase", "mana": mana,
            "per": {"Sum": [{"TargetCount": 0}, -1]}}})
    );
}

#[rstest]
#[case::no_cost("K:Strive")]
#[case::not_mana("K:Strive:Sac<1/Creature>")]
fn a_strive_the_rule_cannot_read_is_a_gap(#[case] line: &str) {
    let text = format!("Name:T\nTypes:Instant\n{line}\n");
    assert_eq!(gaps(&text).len(), 1);
}
