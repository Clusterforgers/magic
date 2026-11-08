//! Goal: "that player" of a tap for mana is `TriggeringPlayer`, never the tapped permanent the event also holds, "its
//! controller adds" keeps the shape of a triggered mana ability, and both translations read back as the printed text.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};

use super::abilities;

fn printed(type_line: &str, oracle: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000f2400".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{G}".into(),
            type_line: type_line.into(),
            oracle_text: oracle.into(),
            ..PrintedFace::default()
        }],
    }
}

#[rstest]
#[case::damage_to_that_player(
    "Name:Alpha\nTypes:Enchantment\nT:Mode$ TapsForMana | ValidCard$ Land | TriggerZones$ Battlefield | Execute$ D\nSVar:D:DB$ DealDamage | Defined$ TriggeredActivator | NumDmg$ 1\n",
    "Enchantment",
    "Whenever a player taps a land for mana, Alpha deals 1 damage to that player.",
    json!({"DealDamage": {"to": "TriggeringPlayer", "amount": 1}})
)]
#[case::mana_for_its_controller(
    "Name:Alpha\nTypes:Enchantment Aura\nK:Enchant:Land\nT:Mode$ TapsForMana | ValidCard$ Card.AttachedBy | Execute$ M | Static$ True\nSVar:M:DB$ Mana | Produced$ G | Amount$ 1 | Defined$ TriggeredCardController\n",
    "Enchantment — Aura",
    "Enchant land\nWhenever enchanted land is tapped for mana, its controller adds an additional {G}.",
    json!({"AddMana": {"who": "Triggering", "mana": {"Fixed": ["Green"]}}})
)]
fn the_player_of_a_tap_for_mana_is_the_player_alone(
    #[case] script: &str,
    #[case] type_line: &str,
    #[case] oracle: &str,
    #[case] effect: Value,
) {
    let found = abilities(script);
    let trigger = found.last().unwrap();
    assert_eq!(trigger["Triggered"]["effects"], json!([effect]));
    let json = json!({"faces": [{"abilities": found}]}).to_string();
    let caps = mtg_engine::caps();
    let names = BTreeSet::from(["alpha".to_owned()]);
    let report = check_exact(
        &json,
        &printed(type_line, oracle),
        &Library::default(),
        &names,
        &caps,
    );
    assert_eq!(report.verdict(), Verdict::Accepted, "{:?}", report.clauses);
}

#[rstest]
#[case::of_a_kind(
    "| ValidCard$ Planeswalker.Wizard | ValidActivatingPlayer$ You",
    "Whenever you activate a loyalty ability of a Wizard planeswalker, draw a card."
)]
#[case::any(
    "| ValidActivatingPlayer$ You",
    "Whenever you activate a loyalty ability, draw a card."
)]
fn a_loyalty_activation_reads_back_as_a_loyalty_ability(#[case] words: &str, #[case] oracle: &str) {
    let script = format!(
        "Name:Alpha\nTypes:Enchantment\nT:Mode$ AbilityCast {words} | ValidSA$ Activated.Loyalty | TriggerZones$ Battlefield | Execute$ D\nSVar:D:DB$ Draw | Defined$ You\n"
    );
    let found = abilities(&script);
    assert_eq!(
        found[0]["Triggered"]["when"]["Did"]["action"],
        json!("ActivatedLoyalty")
    );
    let json = json!({"faces": [{"abilities": found}]}).to_string();
    let caps = mtg_engine::caps();
    let names = BTreeSet::from(["alpha".to_owned()]);
    let report = check_exact(
        &json,
        &printed("Enchantment", oracle),
        &Library::default(),
        &names,
        &caps,
    );
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{json} {:?}",
        report.clauses
    );
}
