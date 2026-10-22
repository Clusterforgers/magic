//! Goal: "As an additional cost to cast this spell, you may ..." reads as an optional additional cost of mana or of the parts an ability pays (a sacrifice, a discard, an exile from a hand, life) and is written back as the same text, and a cost the caster must pay, or one the readers do not model, is left to the table.
use super::*;
use crate::cost::{Cost, CostItem};
use crate::refs::{Pred, Selector};
use crate::value::Amount;
use mtg_core::card::CardType;
use mtg_core::color::Color;

fn of(items: Vec<CostItem>) -> Ability {
    Ability::AdditionalCost { cost: Cost(items) }
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
#[case::mandatory("As an additional cost to cast this spell, discard a card.")]
#[case::mandatory_sacrifice("As an additional cost to cast this spell, sacrifice a creature.")]
#[case::any_number(
    "As an additional cost to cast this spell, you may sacrifice any number of Spirits."
)]
#[case::revealing(
    "As an additional cost to cast this spell, you may reveal a Dragon card from your hand."
)]
fn a_cost_the_caster_must_pay_or_the_readers_do_not_model_stays_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None);
}
