//! The costs a spell must pay in addition (CR 601.2b, 118.8): "As an additional cost to cast this spell, discard a card."
//!
//! They are parts of every way the card is cast, so a cast for an alternative cost pays them too (CR 118.9d), a card whose
//! caster cannot give what they take is not offered (`ways::castable_now`), and the picks are asked with the other parts of
//! the cost (`parts.rs`) and paid with the cast (`cast.rs`). A copy of the spell is not cast and pays nothing (CR 707.10).

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::mana::ManaCost;

use crate::ability_reader;
use crate::parts::Part;
use crate::ways::{self, Way};

/// The parts the required additional costs of the card take, in the order it prints them; `None` when one has a part the
/// announcement does not pay: mana, `{T}`, losing the source, an `{X}` of mana or counters. A part that counts X takes
/// the X the caster announces.
pub(crate) fn parts(card: &CardDefinition) -> Option<Vec<Part>> {
    let mut taken = Vec::new();
    for rule in ability_reader::printed_rules(card) {
        let Ability::AdditionalCost {
            cost,
            required: true,
        } = &**rule
        else {
            continue;
        };
        let (mana, parts) = ways::cast_cost_counting(cost, true)?;
        if mana != ManaCost::ZERO {
            return None;
        }
        taken.extend(parts);
    }
    Some(taken)
}

/// Whether a required additional cost of the card counts the X the caster announces ("pay X life", "sacrifice X creatures").
pub(crate) fn counts_x(card: &CardDefinition) -> bool {
    parts(card).is_some_and(|parts| parts.iter().any(Part::is_counted))
}

/// `ways` with the required additional costs of the card added to each; none when the caster cannot be asked to pay them.
pub(crate) fn add_to(mut ways: Vec<Way>, card: &CardDefinition) -> Vec<Way> {
    let Some(required) = parts(card) else {
        return Vec::new();
    };
    if !required.is_empty() {
        for way in &mut ways {
            way.parts.extend(required.iter().cloned());
        }
    }
    ways
}

#[cfg(test)]
mod tests {
    //! Goal: the required additional costs of a card are the parts it prints in order, added to every way it is cast, and a card with a cost the announcement does not pay has no way to be cast.
    use mtg_cards::cost::{Cost, CostItem};
    use mtg_cards::refs::{Pred, Selector};
    use mtg_cards::value::Amount;
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;

    fn card(abilities: Vec<Ability>) -> CardDefinition {
        abilities
            .into_iter()
            .fold(CardDefinition::new("Test"), CardDefinition::with_rules)
    }

    fn required(items: Vec<CostItem>) -> Ability {
        Ability::AdditionalCost {
            cost: Cost(items),
            required: true,
        }
    }

    fn way(parts: Vec<Part>) -> Way {
        Way {
            alternative: None,
            label: String::new(),
            mana: None,
            instant: false,
            parts,
        }
    }

    fn discard() -> CostItem {
        CostItem::Discard {
            count: Amount::one(),
            filter: None,
        }
    }

    fn sacrifice() -> CostItem {
        CostItem::Sacrifice {
            filter: Selector(vec![Pred::Type(CardType::Creature)]),
            count: Amount::one(),
        }
    }

    #[rstest]
    #[case::none(vec![], 0)]
    #[case::a_discard(vec![required(vec![discard()])], 1)]
    #[case::a_discard_and_a_sacrifice(vec![required(vec![discard(), sacrifice()])], 2)]
    #[case::two_abilities(vec![required(vec![discard()]), required(vec![sacrifice()])], 2)]
    #[case::life(vec![required(vec![CostItem::PayLife(Amount::Fixed(3))])], 1)]
    #[case::an_optional_one_is_not_required(
        vec![Ability::AdditionalCost { cost: Cost(vec![discard()]), required: false }],
        0
    )]
    fn the_parts_are_those_of_the_required_costs_in_print_order(
        #[case] rules: Vec<Ability>,
        #[case] count: usize,
    ) {
        assert_eq!(parts(&card(rules)).map(|parts| parts.len()), Some(count));
    }

    #[rstest]
    #[case::mana(required(vec![CostItem::Mana("{2}".parse().unwrap())]))]
    #[case::tap(required(vec![CostItem::Tap]))]
    #[case::the_source(required(vec![CostItem::SacrificeSelf]))]
    #[case::x_mana(required(vec![CostItem::Mana("{X}".parse().unwrap())]))]
    fn a_cost_the_announcement_does_not_pay_leaves_no_way_to_cast(#[case] rule: Ability) {
        let card = card(vec![rule]);
        assert_eq!(parts(&card), None);
        assert!(
            add_to(vec![way(Vec::new())], &card).is_empty(),
            "no way is left"
        );
    }

    #[rstest]
    #[case::life(CostItem::PayLife(Amount::X), true)]
    #[case::sacrifice(CostItem::Sacrifice {
        filter: Selector(vec![Pred::Type(CardType::Creature)]),
        count: Amount::X,
    }, true)]
    #[case::discard(CostItem::Discard { count: Amount::X, filter: None }, true)]
    #[case::exile(CostItem::ExileFromGraveyard { filter: Selector(Vec::new()), count: Amount::X }, true)]
    #[case::fixed(CostItem::PayLife(Amount::Fixed(3)), false)]
    fn a_part_that_counts_x_is_paid_with_the_x_announced(
        #[case] item: CostItem,
        #[case] counts: bool,
    ) {
        let card = card(vec![required(vec![item])]);
        let read = parts(&card).expect("the cost is paid by the announcement");
        assert_eq!(read.len(), 1);
        assert_eq!(counts_x(&card), counts);
        assert_eq!(read[0].is_counted(), counts);
    }

    #[test]
    fn every_way_takes_the_parts_after_its_own() {
        let own = Part::Life(2);
        let ways = add_to(
            vec![way(Vec::new()), way(vec![own.clone()])],
            &card(vec![required(vec![CostItem::PayLife(Amount::Fixed(3))])]),
        );
        assert_eq!(ways[0].parts, vec![Part::Life(3)]);
        assert_eq!(ways[1].parts, vec![own, Part::Life(3)]);
    }

    #[test]
    fn a_card_with_no_required_cost_keeps_its_ways() {
        let ways = vec![way(Vec::new()), way(vec![Part::Life(1)])];
        assert_eq!(add_to(ways.clone(), &card(Vec::new())), ways);
    }
}
