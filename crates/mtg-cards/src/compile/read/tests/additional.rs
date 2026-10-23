//! Goal: "As an additional cost to cast this spell, [you may] ..." reads as an optional or a required additional cost of mana or of the parts an ability pays (a sacrifice, a discard, an exile from a hand or a graveyard, life) and is written back as the same text, and a cost the readers do not model stays with the table.
use super::*;
use crate::cost::{Cost, CostItem};
use crate::refs::{Pred, Selector};
use crate::value::Amount;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;

fn of(items: Vec<CostItem>) -> Ability {
    Ability::AdditionalCost {
        cost: Cost(items),
        required: false,
    }
}

fn must(items: Vec<CostItem>) -> Ability {
    Ability::AdditionalCost {
        cost: Cost(items),
        required: true,
    }
}

fn kind(card_type: CardType) -> Selector {
    Selector(vec![Pred::Type(card_type)])
}

#[rstest]
#[case::sacrifice(
    "As an additional cost to cast this spell, you may sacrifice an artifact.",
    of(vec![CostItem::Sacrifice { filter: kind(CardType::Artifact), count: Amount::one() }])
)]
#[case::discard(
    "As an additional cost to cast this spell, you may discard a card.",
    of(vec![CostItem::Discard { count: Amount::one(), filter: None }])
)]
#[case::life(
    "As an additional cost to cast this spell, you may pay 2 life.",
    of(vec![CostItem::PayLife(Amount::Fixed(2))])
)]
#[case::mana(
    "As an additional cost to cast this spell, you may pay {2}.",
    of(vec![CostItem::Mana("{2}".parse().unwrap())])
)]
#[case::exile_from_the_hand(
    "As an additional cost to cast this spell, you may exile a red card from your hand.",
    of(vec![CostItem::ExileFromHand {
        filter: Selector(vec![Pred::Color(Color::Red)]),
        count: Amount::one(),
    }])
)]
fn an_optional_additional_cost_reads_and_writes_back(
    #[case] text: &str,
    #[case] expected: Ability,
) {
    assert_eq!(golden(text, ClauseKind::Static, false), expected);
}

#[rstest]
#[case::discard(
    "As an additional cost to cast this spell, discard a card.",
    must(vec![CostItem::Discard { count: Amount::one(), filter: None }])
)]
#[case::discard_two(
    "As an additional cost to cast this spell, discard two cards.",
    must(vec![CostItem::Discard { count: Amount::Fixed(2), filter: None }])
)]
#[case::sacrifice_a_creature(
    "As an additional cost to cast this spell, sacrifice a creature.",
    must(vec![CostItem::Sacrifice { filter: kind(CardType::Creature), count: Amount::one() }])
)]
#[case::sacrifice_a_land(
    "As an additional cost to cast this spell, sacrifice a land.",
    must(vec![CostItem::Sacrifice { filter: kind(CardType::Land), count: Amount::one() }])
)]
#[case::sacrifice_two_creatures(
    "As an additional cost to cast this spell, sacrifice two creatures.",
    must(vec![CostItem::Sacrifice { filter: kind(CardType::Creature), count: Amount::Fixed(2) }])
)]
#[case::life(
    "As an additional cost to cast this spell, pay 3 life.",
    must(vec![CostItem::PayLife(Amount::Fixed(3))])
)]
#[case::exile_from_the_graveyard(
    "As an additional cost to cast this spell, exile a creature card from your graveyard.",
    must(vec![CostItem::ExileFromGraveyard {
        filter: kind(CardType::Creature),
        count: Amount::one(),
    }])
)]
#[case::exile_from_the_hand(
    "As an additional cost to cast this spell, exile a red card from your hand.",
    must(vec![CostItem::ExileFromHand {
        filter: Selector(vec![Pred::Color(Color::Red)]),
        count: Amount::one(),
    }])
)]
fn a_required_additional_cost_reads_and_writes_back(#[case] text: &str, #[case] expected: Ability) {
    assert_eq!(golden(text, ClauseKind::Static, false), expected);
}

#[rstest]
#[case::two_costs_with_and(
    "As an additional cost to cast this spell, discard a card and sacrifice a creature."
)]
#[case::mana_alone("As an additional cost to cast this spell, pay {2}.")]
#[case::any_number(
    "As an additional cost to cast this spell, you may sacrifice any number of Spirits."
)]
#[case::a_list_with_no_or(
    "As an additional cost to cast this spell, discard a card, pay 3 life."
)]
#[case::a_choice_with_a_choice_of_x(
    "As an additional cost to cast this spell, discard X cards or pay 3 life."
)]
#[case::an_optional_choice(
    "As an additional cost to cast this spell, you may discard a card or pay 3 life."
)]
fn a_cost_the_readers_do_not_model_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None);
}

#[rstest]
#[case::discard_or_life(
    "As an additional cost to cast this spell, discard a card or pay 3 life.",
    must(vec![CostItem::OneOf(vec![
        Cost(vec![CostItem::Discard { count: Amount::one(), filter: None }]),
        Cost(vec![CostItem::PayLife(Amount::Fixed(3))]),
    ])])
)]
#[case::three_costs(
    "As an additional cost to cast this spell, sacrifice a creature, discard a card, or pay 4 life.",
    must(vec![CostItem::OneOf(vec![
        Cost(vec![CostItem::Sacrifice { filter: kind(CardType::Creature), count: Amount::one() }]),
        Cost(vec![CostItem::Discard { count: Amount::one(), filter: None }]),
        Cost(vec![CostItem::PayLife(Amount::Fixed(4))]),
    ])])
)]
#[case::a_part_or_mana(
    "As an additional cost to cast this spell, sacrifice a creature or pay {3}.",
    must(vec![CostItem::OneOf(vec![
        Cost(vec![CostItem::Sacrifice { filter: kind(CardType::Creature), count: Amount::one() }]),
        Cost::mana("{3}".parse().unwrap()),
    ])])
)]
#[case::a_reveal_or_mana(
    "As an additional cost to cast this spell, reveal a Dinosaur card from your hand or pay {1}.",
    must(vec![CostItem::OneOf(vec![
        Cost(vec![CostItem::RevealFromHand {
            filter: Selector(vec![Pred::Subtype(Subtype("Dinosaur".to_owned()))]),
            count: Amount::one(),
        }]),
        Cost::mana("{1}".parse().unwrap()),
    ])])
)]
fn a_choice_between_required_additional_costs_reads_and_writes_back(
    #[case] text: &str,
    #[case] expected: Ability,
) {
    assert_eq!(golden(text, ClauseKind::Static, false), expected);
}

#[rstest]
#[case::a_creature_card(
    "As an additional cost to cast this spell, reveal a creature card from your hand.",
    must(vec![CostItem::RevealFromHand { filter: kind(CardType::Creature), count: Amount::one() }])
)]
#[case::a_colorless_creature_card(
    "As an additional cost to cast this spell, reveal a colorless creature card from your hand.",
    must(vec![CostItem::RevealFromHand {
        filter: Selector(vec![Pred::Type(CardType::Creature), Pred::Colorless]),
        count: Amount::one(),
    }])
)]
#[case::two_cards(
    "As an additional cost to cast this spell, reveal two creature cards from your hand.",
    must(vec![CostItem::RevealFromHand { filter: kind(CardType::Creature), count: Amount::Fixed(2) }])
)]
#[case::optional(
    "As an additional cost to cast this spell, you may reveal a Dragon card from your hand.",
    of(vec![CostItem::RevealFromHand {
        filter: Selector(vec![Pred::Subtype(Subtype("Dragon".to_owned()))]),
        count: Amount::one(),
    }])
)]
fn a_reveal_from_the_hand_as_an_additional_cost_reads_and_writes_back(
    #[case] text: &str,
    #[case] expected: Ability,
) {
    assert_eq!(golden(text, ClauseKind::Static, false), expected);
}

#[rstest]
#[case::life(
    "As an additional cost to cast this spell, pay X life.",
    must(vec![CostItem::PayLife(Amount::X)])
)]
#[case::sacrifice(
    "As an additional cost to cast this spell, sacrifice X creatures.",
    must(vec![CostItem::Sacrifice { filter: kind(CardType::Creature), count: Amount::X }])
)]
#[case::sacrifice_a_kind(
    "As an additional cost to cast this spell, sacrifice X artifacts.",
    must(vec![CostItem::Sacrifice { filter: kind(CardType::Artifact), count: Amount::X }])
)]
#[case::discard(
    "As an additional cost to cast this spell, discard X cards.",
    must(vec![CostItem::Discard { count: Amount::X, filter: None }])
)]
#[case::exile_from_the_graveyard(
    "As an additional cost to cast this spell, exile X cards from your graveyard.",
    must(vec![CostItem::ExileFromGraveyard { filter: Selector(Vec::new()), count: Amount::X }])
)]
#[case::exile_a_kind_from_the_graveyard(
    "As an additional cost to cast this spell, exile X creature cards from your graveyard.",
    must(vec![CostItem::ExileFromGraveyard { filter: kind(CardType::Creature), count: Amount::X }])
)]
fn a_required_additional_cost_that_counts_x_reads_and_writes_back(
    #[case] text: &str,
    #[case] expected: Ability,
) {
    assert_eq!(golden(text, ClauseKind::Static, false), expected);
}

#[rstest]
#[case::optional_life("As an additional cost to cast this spell, you may pay X life.")]
#[case::optional("As an additional cost to cast this spell, you may sacrifice X artifacts.")]
#[case::optional_discard("As an additional cost to cast this spell, you may discard X cards.")]
#[case::mana("As an additional cost to cast this spell, you may pay {X}.")]
#[case::mana_alone("As an additional cost to cast this spell, pay {X}.")]
#[case::mana_and_life("As an additional cost to cast this spell, pay {X} and 2 life.")]
#[case::x_with_a_number("As an additional cost to cast this spell, pay X plus 2 life.")]
#[case::half_of_it("As an additional cost to cast this spell, pay half X life.")]
fn an_x_in_the_additional_cost_that_the_caster_does_not_announce_stays_with_the_table(
    #[case] text: &str,
) {
    assert_eq!(clause(text, ClauseKind::Static, false), None);
}
