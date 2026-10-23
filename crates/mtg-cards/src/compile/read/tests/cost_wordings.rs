//! Goal: the costs of activated abilities and spells that name a kind of card or permanent read as printed and write back as the same text: a discard of a kind of card, a sacrifice of a creature with a subtype, and a run of sacrifices "a Swamp and a Forest"; near-misses stay with the table.
use super::*;
use crate::cost::{Cost, CostItem};
use crate::refs::{Pred, Selector};
use crate::value::Amount;
use mtg_core::card::{CardType, Subtype};

fn kind(card_type: CardType) -> Selector {
    Selector(vec![Pred::Type(card_type)])
}

fn subtype(name: &str) -> Selector {
    Selector(vec![Pred::Subtype(Subtype(name.to_owned()))])
}

fn cost_of(read: &Ability) -> &Cost {
    match read {
        Ability::Activated { cost, .. } | Ability::AdditionalCost { cost, .. } => cost,
        other => panic!("not an ability with a cost: {other:?}"),
    }
}

#[rstest]
#[case::a_land_card(
    "{1}{R}, Discard a land card: ~ deals 2 damage to any target.",
    CostItem::Discard { count: Amount::one(), filter: Some(kind(CardType::Land)) }
)]
#[case::a_creature_card(
    "{G}, Discard a creature card: Draw a card.",
    CostItem::Discard { count: Amount::one(), filter: Some(kind(CardType::Creature)) }
)]
#[case::two_creature_cards(
    "{G}, Discard two creature cards: Draw a card.",
    CostItem::Discard { count: Amount::Fixed(2), filter: Some(kind(CardType::Creature)) }
)]
#[case::a_plain_card(
    "{G}, Discard a card: Draw a card.",
    CostItem::Discard { count: Amount::one(), filter: None }
)]
fn a_discard_names_the_kind_of_card(#[case] text: &str, #[case] item: CostItem) {
    let read = golden(text, ClauseKind::Activated, false);
    assert!(cost_of(&read).0.contains(&item), "{read:?}");
}

#[rstest]
#[case::a_land_card("As an additional cost to cast this spell, discard a land card.")]
#[case::two_creature_cards("As an additional cost to cast this spell, discard two creature cards.")]
fn an_additional_discard_names_the_kind_of_card(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, false);
    assert!(matches!(
        cost_of(&read).0.as_slice(),
        [CostItem::Discard {
            filter: Some(_),
            ..
        }]
    ));
}

#[rstest]
#[case::a_land("{G}, Discard a land: Draw a card.")]
#[case::a_card_in_a_zone("{G}, Discard a land card from your hand: Draw a card.")]
fn a_discard_that_does_not_say_card_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Activated, false), None);
}

#[rstest]
#[case::a_cleric_creature(
    "{B}, {T}, Sacrifice a Cleric creature: Target player loses life equal to the sacrificed creature's power.",
    Selector(vec![Pred::Type(CardType::Creature), Pred::Subtype(Subtype("Cleric".to_owned()))])
)]
#[case::a_goblin("{R}, Sacrifice a Goblin: Draw a card.", subtype("Goblin"))]
fn a_sacrifice_names_a_subtype_with_or_without_a_type(
    #[case] text: &str,
    #[case] filter: Selector,
) {
    let read = golden(text, ClauseKind::Activated, false);
    assert!(
        cost_of(&read).0.contains(&CostItem::Sacrifice {
            filter,
            count: Amount::one()
        }),
        "{read:?}"
    );
}

#[rstest]
#[case::a_swamp_and_a_forest("Sacrifice a Swamp and a Forest: Draw a card.", &["Swamp", "Forest"])]
#[case::with_mana("{1}, Sacrifice a Swamp and a Forest: Draw a card.", &["Swamp", "Forest"])]
fn a_run_of_sacrifices_is_one_part_for_each_permanent(#[case] text: &str, #[case] names: &[&str]) {
    let read = golden(text, ClauseKind::Activated, false);
    let sacrifices: Vec<Selector> = cost_of(&read)
        .0
        .iter()
        .filter_map(|item| match item {
            CostItem::Sacrifice { filter, count } if *count == Amount::one() => {
                Some(filter.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        sacrifices,
        names.iter().map(|name| subtype(name)).collect::<Vec<_>>()
    );
}

#[rstest]
#[case::the_trigger_of_a_group(
    "Whenever ~ or another Human creature you control dies, draw a card."
)]
#[case::a_spell_of_a_kind("Counter target Human creature spell.")]
#[case::a_target("Destroy target Cleric creature.")]
fn a_subtype_before_a_type_in_the_singular_reads_in_every_phrase(#[case] text: &str) {
    let kind = if text.starts_with("Whenever") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    };
    golden(text, kind, false);
}

#[rstest]
#[case::another_after_and("Sacrifice a Swamp and another creature: Draw a card.")]
#[case::a_count_after_and("Sacrifice a Swamp and two Forests: Draw a card.")]
fn a_run_of_sacrifices_takes_single_permanents_only(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Activated, false), None);
}
