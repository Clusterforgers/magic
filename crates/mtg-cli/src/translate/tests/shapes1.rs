//! Goal: "any combination of colors" is a counted pick of the engine and plays as one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use super::abilities;

fn artifact(line: &str) -> String {
    format!("Name:Subject\nManaCost:2\nTypes:Artifact\n{line}\n")
}

#[rstest]
#[case::two("Amount$ 2", json!(2))]
#[case::five("Amount$ 5", json!(5))]
fn mana_in_any_combination_is_a_counted_pick(#[case] amount: &str, #[case] count: Value) {
    let text = artifact(&format!(
        "A:AB$ Mana | Cost$ T | Produced$ Combo Any | {amount}"
    ));
    let found = abilities(&text);
    let mana = &found[0]["Activated"]["effects"][0]["AddMana"]["mana"];
    assert_eq!(mana["AnyCombination"]["count"], count, "{found:?}");
}

#[rstest]
#[case::one_stays_a_colour("Produced$ Combo Any | Amount$ 1")]
#[case::any_amount_two_is_one_colour_each("Produced$ Any | Amount$ 2")]
fn a_single_mana_or_a_run_of_one_colour_stays_as_it_was(#[case] line: &str) {
    let found = abilities(&artifact(&format!("A:AB$ Mana | Cost$ T | {line}")));
    assert!(found[0]["Activated"]["effects"][0]["AddMana"]["mana"]["AnyColor"].is_number());
}

fn cache(spell_cost: &str) -> Driver {
    let line = "A:AB$ Mana | Cost$ T | Produced$ Combo Any | Amount$ 2";
    let mut card = CardDefinition::new("Cache").with_type(CardType::Artifact);
    for value in abilities(&artifact(line)) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    let spell = CardDefinition::new("Spell")
        .with_type(CardType::Instant)
        .with_mana_cost(ManaCost::parse(spell_cost).unwrap());
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [Arc::new(spell)])
            .battlefield(0, [ready(Arc::new(card))])
            .library(0, (0..12).map(|_| mountain()))
            .library(1, (0..12).map(|_| mountain())),
    )
    .unwrap()
}

#[rstest]
#[case::two_colours("{R}{G}", true)]
#[case::one_colour_twice("{U}{U}", true)]
#[case::three_is_one_too_many("{R}{G}{B}", false)]
fn a_translated_combination_pays_for_two_colours(#[case] cost: &str, #[case] castable: bool) {
    let mut driver = cache(cost);
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    assert_eq!(!options.castable.is_empty(), castable);
    if castable {
        driver.cast("Spell").unwrap();
        let pool = driver.state().player(PlayerId::from_index(0)).unwrap();
        assert_eq!(pool.mana().total(), 0);
    }
}
