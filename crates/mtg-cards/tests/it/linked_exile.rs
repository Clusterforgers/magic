//! Goal: the two triggers of Oblivion Ring, Journey to Nowhere, Fiend Hunter and Leonin Relic-Warder read as one exile that lasts until the card leaves the battlefield, and a return trigger without such an exile before it stays with the table.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::card::ImplStatus;
use mtg_cards::compile::compile;
use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_cards::support::{PrintedCard, PrintedFace};
use rstest::rstest;

const RETURN: &str = "When this permanent leaves the battlefield, return the exiled card to the battlefield under its owner's control.";

fn card(name: &str, oracle_id: &str, type_line: &str, text: &str) -> PrintedCard {
    PrintedCard {
        layout: "normal".into(),
        oracle_id: oracle_id.into(),
        name: name.into(),
        keywords: vec![],
        faces: vec![PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            oracle_text: text.into(),
            ..PrintedFace::default()
        }],
    }
}

/// The durations of the exiles the abilities run, inside a `May` too.
fn exile_durations(abilities: &[Ability]) -> Vec<Option<Duration>> {
    fn walk(effects: &[Effect], found: &mut Vec<Option<Duration>>) {
        for effect in effects {
            match effect {
                Effect::Exile { duration, .. } => found.push(*duration),
                Effect::May { effects, .. } => walk(effects, found),
                _ => {}
            }
        }
    }
    let mut found = Vec::new();
    for ability in abilities {
        walk(ability.effects(), &mut found);
    }
    found
}

#[rstest]
#[case::oblivion_ring(
    "Oblivion Ring",
    "bd9b9772-f5f9-4c6b-913e-7193bea5d0a7",
    "Enchantment",
    "When this enchantment enters, exile another target nonland permanent.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::journey_to_nowhere(
    "Journey to Nowhere",
    "2bfec482-be0d-4768-84e7-1c2c74ea1373",
    "Enchantment",
    "When this enchantment enters, exile target creature.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::fiend_hunter(
    "Fiend Hunter",
    "cb9d557a-fc06-428c-8be6-7d28add33028",
    "Creature — Human Cleric",
    "When this creature enters, you may exile another target creature.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::leonin_relic_warder(
    "Leonin Relic-Warder",
    "8e720216-a980-4a34-8aef-62ac3ef15a6e",
    "Creature — Cat Cleric",
    "When this creature enters, you may exile target artifact or enchantment.\nWhen this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
fn the_enter_and_leave_triggers_are_one_exile_that_lasts_until_the_card_leaves(
    #[case] name: &str,
    #[case] oracle_id: &str,
    #[case] type_line: &str,
    #[case] text: &str,
) {
    let printed = card(name, oracle_id, type_line, text);
    let compiled = compile(&printed);
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_eq!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    assert!(compiled.clauses.iter().all(|clause| clause.read));
    assert_eq!(made.faces[0].abilities.len(), 1, "{:?}", made.faces[0]);
    assert_eq!(
        exile_durations(&made.faces[0].abilities),
        vec![Some(Duration::WhileSourceOnBattlefield)]
    );
}

#[rstest]
#[case::no_exile_before_it(
    "Enchantment",
    "When this enchantment enters, draw a card.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::the_return_alone(
    "Enchantment",
    "When this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::an_exile_that_is_not_on_entering(
    "Enchantment",
    "Whenever you cast a spell, exile target creature.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
#[case::an_exile_that_already_lasts(
    "Enchantment",
    "When this enchantment enters, exile target creature until this enchantment leaves the battlefield.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control."
)]
fn a_return_trigger_without_the_exile_of_the_card_stays_with_the_table(
    #[case] type_line: &str,
    #[case] text: &str,
) {
    let printed = card(
        "Test Warden",
        "4457ed35-7c10-48c8-9776-456485fdf070",
        type_line,
        text,
    );
    let compiled = compile(&printed);
    let made = compiled.card.as_ref().expect("the card compiles");
    assert_ne!(made.impl_status, ImplStatus::Full, "{compiled:?}");
    let returns = compiled
        .clauses
        .iter()
        .filter(|clause| clause.text.contains("return the exiled card"))
        .collect::<Vec<_>>();
    assert_eq!(returns.len(), 1, "{compiled:?}");
    assert!(!returns[0].read, "{compiled:?} ({RETURN})");
}
