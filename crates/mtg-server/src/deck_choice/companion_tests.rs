//! Goal: the game's format judges a seat's companion after its deck, in the companion zone and with the code of the deck check: a Normal game by the condition over the 60 cards, a Commander game by the condition over the 100 cards with the commander and by the commanders' colour identity unless the table relaxed it; a card without a companion ability is no companion, and a deck that breaks its own rules first is told only that.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::deck::{DeckZone, IssueCode};
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::house::{DeckRules, HouseRules};
use mtg_core::mana::ManaCost;
use mtg_engine::rng::GameRng;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::companion_cards::{
    companion_with, lands_and_big_spells, small_companion, small_deck,
};
use rstest::rstest;

use super::*;
use crate::demo;

fn normal_deck() -> Vec<Arc<CardDefinition>> {
    small_deck(&mut GameRng::substream(5, 1))
}

/// A red creature with a companion ability that "Keruga" style decks meet.
fn red_companion() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Red Companion")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse("{1}{R}").unwrap())
            .with_power_toughness(2, 2)
            .with_rules(Ability::Keyword(Keyword::companion(lands_and_big_spells()))),
    )
}

fn no_ability() -> Arc<CardDefinition> {
    creature("Plain Bear", 1, &[Color::Green], 2, 2)
}

fn big_companion() -> Arc<CardDefinition> {
    companion_with("Big Companion", lands_and_big_spells())
}

fn judged(
    format: FormatKind,
    library: &[Arc<CardDefinition>],
    commanders: &[Arc<CardDefinition>],
    companion: &Arc<CardDefinition>,
    house: &HouseRules,
) -> Vec<(IssueCode, Option<DeckZone>)> {
    format_issues(format, library, commanders, Some(companion), house)
        .into_iter()
        .map(|issue| (issue.code, issue.zone))
        .collect()
}

const IN_ZONE: (IssueCode, Option<DeckZone>) =
    (IssueCode::CompanionCondition, Some(DeckZone::Companion));

#[rstest]
#[case::the_deck_meets_the_condition(small_companion(), vec![])]
#[case::the_deck_breaks_the_condition(big_companion(), vec![IN_ZONE])]
#[case::the_card_has_no_companion_ability(no_ability(), vec![IN_ZONE])]
fn a_normal_game_judges_the_companion_by_the_condition_over_the_deck(
    #[case] companion: Arc<CardDefinition>,
    #[case] expected: Vec<(IssueCode, Option<DeckZone>)>,
) {
    let got = judged(
        FormatKind::Normal,
        &normal_deck(),
        &[],
        &companion,
        &HouseRules::default(),
    );
    assert_eq!(got, expected);
}

#[test]
fn a_seat_without_a_companion_has_nothing_to_judge_and_one_that_breaks_the_deck_rules_is_told_that_first()
 {
    let none = format_issues(
        FormatKind::Normal,
        &normal_deck(),
        &[],
        None,
        &HouseRules::default(),
    );
    assert!(none.is_empty());
    let few = &normal_deck()[..40];
    let issues = judged(
        FormatKind::Normal,
        few,
        &[],
        &big_companion(),
        &HouseRules::default(),
    );
    assert_eq!(issues, [(IssueCode::DeckSize, Some(DeckZone::Main))]);
    let commanded = judged(
        FormatKind::Normal,
        &normal_deck(),
        &[forest()],
        &big_companion(),
        &HouseRules::default(),
    );
    assert_eq!(
        commanded,
        [(IssueCode::NoCommanderZone, Some(DeckZone::Commander))]
    );
}

fn relaxed(set: impl FnOnce(&mut DeckRules)) -> HouseRules {
    let mut deck = DeckRules::default();
    set(&mut deck);
    HouseRules {
        deck,
        ..HouseRules::default()
    }
}

#[rstest]
#[case::inside_the_identity(big_companion(), HouseRules::default(), vec![])]
#[case::the_condition_is_over_all_hundred_cards(small_companion(), HouseRules::default(), vec![IN_ZONE])]
#[case::outside_the_identity(red_companion(), HouseRules::default(), vec![(IssueCode::ColorIdentity, Some(DeckZone::Companion))])]
#[case::outside_but_the_table_relaxed_the_identity(red_companion(), relaxed(|d| d.colour_identity = Some(false)), vec![])]
#[case::another_relaxation_leaves_the_identity(red_companion(), relaxed(|d| d.singleton = Some(false)), vec![(IssueCode::ColorIdentity, Some(DeckZone::Companion))])]
#[case::no_companion_ability(no_ability(), HouseRules::default(), vec![IN_ZONE])]
fn a_commander_game_judges_the_condition_over_the_commanders_deck_and_the_identity(
    #[case] companion: Arc<CardDefinition>,
    #[case] house: HouseRules,
    #[case] expected: Vec<(IssueCode, Option<DeckZone>)>,
) {
    let (commander, library) = demo::commander_stand_in_of(100);
    let got = judged(
        FormatKind::Commander,
        &library,
        std::slice::from_ref(&commander),
        &companion,
        &house,
    );
    assert_eq!(got, expected);
}
