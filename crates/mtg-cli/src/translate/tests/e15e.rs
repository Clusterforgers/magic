//! Goal: Forge's recover, ripple and epic lines read as the engine's keywords, and their Oracle lines read back to the same keyword while another cost, number or keyword reads `Differs`.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};

use super::{abilities, gaps};

#[rstest]
#[case::recover("Recover:R R", json!({"Recover": "{R}{R}"}))]
#[case::recover_generic("Recover:2 U U", json!({"Recover": "{2}{U}{U}"}))]
#[case::ripple("Ripple:4", json!({"Ripple": 4}))]
#[case::epic("Epic", json!("Epic"))]
fn the_keywords_read_from_their_forge_lines(#[case] line: &str, #[case] keyword: Value) {
    let script = format!("Name:T\nTypes:Sorcery\nK:{line}\n");
    assert_eq!(gaps(&script), Vec::<String>::new());
    assert_eq!(abilities(&script), vec![json!({ "Keyword": keyword })]);
}

const RECOVER: &str = "Recover {R}{R} (When a creature is put into your graveyard from the battlefield, you may pay {R}{R}. If you do, return this card from your graveyard to your hand. Otherwise, exile this card.)";
const RIPPLE: &str = "Ripple 4 (When you cast this spell, you may reveal the top four cards of your library. You may cast spells with the same name as this spell from among those cards without paying their mana costs. Put the rest on the bottom of your library.)";
const EPIC: &str = "Epic (For the rest of the game, you can't cast spells. At the beginning of each of your upkeeps, copy this spell except for its epic ability.)";

fn printed(oracle: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000e15e0".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{R}".into(),
            type_line: "Sorcery".into(),
            oracle_text: format!("Draw a card.\n{oracle}"),
            ..PrintedFace::default()
        }],
    }
}

#[rstest]
#[case::recover(RECOVER, json!({"Recover": "{R}{R}"}), Verdict::Accepted)]
#[case::recover_another_cost(RECOVER, json!({"Recover": "{R}"}), Verdict::Mismatch)]
#[case::recover_another_keyword(RECOVER, json!({"Echo": "{R}{R}"}), Verdict::Mismatch)]
#[case::ripple(RIPPLE, json!({"Ripple": 4}), Verdict::Accepted)]
#[case::ripple_another_number(RIPPLE, json!({"Ripple": 3}), Verdict::Mismatch)]
#[case::epic(EPIC, json!("Epic"), Verdict::Accepted)]
#[case::epic_another_keyword(EPIC, json!("Storm"), Verdict::Mismatch)]
fn the_oracle_lines_read_back_to_the_same_keyword(
    #[case] oracle: &str,
    #[case] keyword: Value,
    #[case] verdict: Verdict,
) {
    let card = printed(oracle);
    let caps = mtg_engine::caps();
    let names = BTreeSet::from(["alpha".to_owned()]);
    let json = json!({"faces": [{"abilities": [{"Spell": {"effects": [{"Draw": {}}]}}, {"Keyword": keyword}]}]})
        .to_string();
    let report = check_exact(&json, &card, &Library::default(), &names, &caps);
    assert_eq!(report.verdict(), verdict, "{:?}", report.clauses);
}
