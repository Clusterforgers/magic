//! Goal: the engine pieces of round 5 pack F are read from Forge scripts: freerunning with its cost is an alternative cost keyword, a choice each player makes among their own reads `Chooser`, and a form the reader does not know stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn keyword(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nK:{line}\nA:SP$ Draw | NumCards$ 1\n")
}

#[rstest]
#[case::freerunning("Freerunning:1 B", json!({"Freerunning": "{1}{B}"}))]
#[case::freerunning_hybrid("Freerunning:U B", json!({"Freerunning": "{U}{B}"}))]
fn freerunning_is_read_with_its_cost(#[case] line: &str, #[case] expected: Value) {
    let text = keyword(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0]["Keyword"], expected);
}

#[rstest]
#[case::no_cost("Freerunning")]
fn freerunning_without_a_cost_stays_a_gap(#[case] line: &str) {
    assert_ne!(gaps(&keyword(line)), Vec::<String>::new());
}

fn choose(extra: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ ChooseCard | Defined$ Player | Choices$ Creature | ControlledByPlayer$ Chooser{extra} | SubAbility$ DBDestroy\nSVar:DBDestroy:DB$ DestroyAll | ValidCards$ Creature.nonChosenCard\n"
    )
}

#[rstest]
#[case::battlefield("", "ControlledBy")]
#[case::hand(" | ChoiceZone$ Hand", "OwnedBy")]
#[case::graveyard(" | ChoiceZone$ Graveyard", "OwnedBy")]
fn each_player_chooses_among_their_own(#[case] extra: &str, #[case] whose: &str) {
    let text = choose(extra);
    let effects = &abilities(&text)[0]["Spell"]["effects"];
    let chosen = &effects[0]["Choose"];
    assert_eq!(chosen["who"], json!("EachPlayer"));
    let filter = chosen["from"]["Each"].as_array().unwrap();
    assert!(filter.contains(&json!({ whose: "Chooser" })), "{filter:?}");
}

#[rstest]
#[case::at_random(" | AtRandom$ True", "key:ChooseCard.ControlledByPlayer=Chooser")]
#[case::exile(
    " | ChoiceZone$ Exile",
    "key:ChooseCard.ControlledByPlayer=Chooser-zone"
)]
fn a_chooser_without_own_cards_stays_a_gap(#[case] extra: &str, #[case] gap: &str) {
    let found = gaps(&choose(extra));
    assert!(found.contains(&gap.to_owned()), "{found:?}");
}
