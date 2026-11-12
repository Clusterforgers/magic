//! Goal: `Pred::SharesCreatureTypeWithEach` holds for a card in hand that shares a creature type with every creature
//! the reference names, one to three of them, fails when one shares none, counts a changeling as every creature type on
//! either side, and holds when the reference names nothing.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::Scope;
use crate::fixtures::setup;
use crate::state::GameState;
use crate::zones::Zone;

/// A 1/1 creature with these creature types, a changeling when the list is `["Changeling"]`.
fn creature(name: &str, types: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1);
    for kind in types {
        card = if *kind == "Changeling" {
            card.with_rules(Ability::Keyword(Keyword::Changeling))
        } else {
            card.with_subtype(*kind)
        };
    }
    Arc::new(card)
}

/// Whether the card in hand shares a creature type with each creature on the battlefield.
fn shares(battlefield: &[&[&str]], hand: &[&str]) -> bool {
    let field = battlefield
        .iter()
        .enumerate()
        .map(|(i, types)| creature(&format!("Field {i}"), types))
        .collect();
    let setup = setup(
        vec![vec![creature("Card", hand)], vec![]],
        vec![field, vec![]],
    );
    let game = GameState::from_setup(&setup).unwrap();
    let card = game.zone(Zone::Hand(PlayerId(0)))[0];
    let source = game.object(card).unwrap().reference();
    let selector = Selector(vec![
        Pred::In(ZoneKind::Hand),
        Pred::SharesCreatureTypeWithEach(Ref::Each(Selector(vec![Pred::Type(CardType::Creature)]))),
    ]);
    Scope::announcing(&game, PlayerId(0), source)
        .matching(&selector)
        .unwrap()
        .iter()
        .any(|found| found.id == card)
}

#[rstest]
#[case::one_shared(vec![vec!["Elf", "Warrior"]], vec!["Warrior"], true)]
#[case::shared_with_both(vec![vec!["Elf", "Warrior"], vec!["Goblin", "Warrior"]], vec!["Warrior"], true)]
#[case::shared_with_one_only(vec![vec!["Elf", "Warrior"], vec!["Goblin", "Shaman"]], vec!["Warrior"], false)]
#[case::a_type_for_each(vec![vec!["Elf"], vec!["Goblin"], vec!["Human"]], vec!["Elf", "Goblin", "Human"], true)]
#[case::none_shared(vec![vec!["Elf"]], vec!["Goblin"], false)]
#[case::changeling_in_hand(vec![vec!["Elf"], vec!["Goblin"]], vec!["Changeling"], true)]
#[case::changeling_on_the_battlefield(vec![vec!["Changeling"], vec!["Goblin"]], vec!["Goblin"], true)]
#[case::changeling_and_no_type(vec![vec!["Changeling"]], vec![], false)]
#[case::nothing_named(vec![], vec!["Elf"], true)]
fn a_card_shares_a_creature_type_with_each(
    #[case] battlefield: Vec<Vec<&str>>,
    #[case] hand: Vec<&str>,
    #[case] expected: bool,
) {
    let field: Vec<&[&str]> = battlefield.iter().map(Vec::as_slice).collect();
    assert_eq!(shares(&field, &hand), expected);
}
