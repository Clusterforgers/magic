//! Goal: the rows of the table of a die roll are read after the roll of a spell, whatever the number of rows, and a row
//! the card does not print, or that no writer says, is never read as the card's.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};

fn verdict(oracle: &str, abilities: &Value) -> Verdict {
    let card = PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000f1800".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{2}{U}".into(),
            type_line: "Sorcery".into(),
            oracle_text: oracle.into(),
            ..PrintedFace::default()
        }],
    };
    let names = BTreeSet::from(["alpha".to_owned()]);
    let json = json!({"faces": [{"abilities": abilities}]}).to_string();
    check_exact(
        &json,
        &card,
        &Library::default(),
        &names,
        &mtg_engine::caps(),
    )
    .verdict()
}

fn contact() -> Value {
    json!([{"Spell": {"effects": [{"RollDice": {"sides": 20, "results": [
        {"from": 1, "to": 9, "effects": [{"Draw": {"count": 2}}]},
        {"from": 10, "to": 19, "effects": [{"Scry": {"count": 2}}, {"Draw": {"count": 2}}]},
        {"from": 20, "to": 20, "effects": [{"Scry": {"count": 3}}, {"Draw": {"count": 3}}]}
    ]}}]}}])
}

fn cone() -> Value {
    let tap =
        json!({"Tap": {"what": {"Each": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}}});
    let stay = json!({"Continuous": {"affected": {"Each": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]},
        "duration": "NextUntapStep", "mods": ["DoesntUntap"]}});
    let enter = json!({"Shield": {"by": "Tapped", "duration": "UntilYourNextTurn",
        "when": {"Enters": {"what": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}}}});
    json!([{"Spell": {"effects": [{"RollDice": {"sides": 20, "results": [
        {"from": 1, "to": 9, "effects": [tap]},
        {"from": 10, "to": 19, "effects": [tap, stay]},
        {"from": 20, "to": 20, "effects": [tap, stay, enter]}
    ]}}]}}])
}

const CONTACT: &str = "Roll a d20.\n1\u{2014}9 | Draw two cards.\n10\u{2014}19 | Scry 2, then draw two cards.\n20 | Scry 3, then draw three cards.";
const CONE: &str = "Roll a d20.\n1\u{2014}9 | Tap all creatures your opponents control.\n10\u{2014}19 | Tap all creatures your opponents control. Those creatures don't untap during their controllers' next untap steps.\n20 | Tap all creatures your opponents control. Those creatures don't untap during their controllers' next untap steps. Until your next turn, creatures your opponents control enter tapped.";

#[rstest]
#[case::the_rows_of_the_card(CONTACT, Verdict::Accepted)]
#[case::another_row(
    "Roll a d20.\n1\u{2014}9 | Draw three cards.\n10\u{2014}19 | Scry 2, then draw two cards.\n20 | Scry 3, then draw three cards.",
    Verdict::Mismatch
)]
#[case::another_last_row(
    "Roll a d20.\n1\u{2014}9 | Draw two cards.\n10\u{2014}19 | Scry 2, then draw two cards.\n20 | Scry 3, then draw four cards.",
    Verdict::Mismatch
)]
fn a_spell_that_rolls_reads_its_rows_after_the_roll(
    #[case] oracle: &str,
    #[case] expected: Verdict,
) {
    assert_eq!(verdict(oracle, &contact()), expected);
}

#[rstest]
#[case::a_row_without_a_writer(CONE, false)]
#[case::a_row_of_another_kind(
    "Roll a d20.\n1\u{2014}9 | Tap all creatures you control.\n10\u{2014}19 | Tap all creatures your opponents control. Those creatures don't untap during their controllers' next untap steps.\n20 | Tap all creatures your opponents control.",
    true
)]
fn a_row_no_writer_says_is_left_to_the_table(#[case] oracle: &str, #[case] mismatch: bool) {
    let found = verdict(oracle, &cone());
    assert_eq!(found == Verdict::Mismatch, mismatch, "{found:?}");
}
