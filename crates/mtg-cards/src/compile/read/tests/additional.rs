//! Goal: "As an additional cost to cast this spell, [you may] ..." reads as an optional or a required additional cost of mana or of the parts an ability pays (a sacrifice, a discard, an exile from a hand or a graveyard, life) and is written back as the same text, and a cost the readers do not model stays with the table.
use super::*;
use crate::cost::{Cost, CostItem};
use crate::refs::{Pred, Selector};
use crate::value::Amount;
use mtg_core::card::CardType;
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
#[case::either_of_two_costs(
    "As an additional cost to cast this spell, discard a card or pay 3 life."
)]
#[case::a_cost_or_mana(
    "As an additional cost to cast this spell, sacrifice a creature or pay {3}."
)]
#[case::two_costs_with_and(
    "As an additional cost to cast this spell, discard a card and sacrifice a creature."
)]
#[case::mana_alone("As an additional cost to cast this spell, pay {2}.")]
#[case::any_number(
    "As an additional cost to cast this spell, you may sacrifice any number of Spirits."
)]
#[case::revealing(
    "As an additional cost to cast this spell, you may reveal a Dragon card from your hand."
)]
#[case::revealing_a_required_card(
    "As an additional cost to cast this spell, reveal a creature card from your hand."
)]
fn a_cost_the_readers_do_not_model_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None);
}

#[rstest]
#[case::life("As an additional cost to cast this spell, pay X life.")]
#[case::sacrifice("As an additional cost to cast this spell, sacrifice X creatures.")]
#[case::discard("As an additional cost to cast this spell, discard X cards.")]
#[case::optional("As an additional cost to cast this spell, you may sacrifice X artifacts.")]
fn an_x_in_the_additional_cost_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None);
}
