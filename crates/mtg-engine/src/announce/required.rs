//! The costs a spell must pay in addition (CR 601.2b, 118.8): "As an additional cost to cast this spell, discard a card."
//!
//! They are parts of every way the card is cast, so a cast for an alternative cost pays them too (CR 118.9d), a card whose
//! caster cannot give what they take is not offered (`ways::castable_now`), and the picks are asked with the other parts of
//! the cost (`parts.rs`) and paid with the cast (`cast.rs`). A copy of the spell is not cast and pays nothing (CR 707.10).
//!
//! A cost that lets the caster choose ("discard a card or pay 3 life", `CostItem::OneOf`) makes one way of each way for each
//! cost to choose, so the way chosen with `ChooseOption` is also the cost chosen (`Way::variant`, kept in
//! `CostTaken::variant`), and a card is offered when one of them can be paid. Mana in a required cost joins the mana of the way.

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::{Cost, CostItem};
use mtg_core::mana::ManaCost;

use crate::ability_reader;
use crate::parts::Part;
use crate::ways::{self, Way};

/// One way to pay the required additional costs of a card: the mana and the parts the costs take, and the costs chosen in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Variant {
    /// The mana the costs add to the cost of the way.
    pub(crate) mana: ManaCost,
    /// The parts the costs take, in the order the card prints them.
    pub(crate) parts: Vec<Part>,
    /// What the caster chose, for the label of the way; empty when the card lets them choose nothing.
    words: Vec<String>,
}

/// The costs a required additional cost lets the caster choose between: the alternatives of a choice, else the cost itself.
fn alternatives(cost: &Cost) -> Vec<&Cost> {
    match cost.0.as_slice() {
        [CostItem::OneOf(costs)] => costs.iter().collect(),
        _ => vec![cost],
    }
}

/// The ways to pay the required additional costs of the card, one for each combination of the costs it lets the caster
/// choose between, in the order it prints them (a single one with nothing to pay for a card that has none); `None` when one
/// has a part the announcement does not pay: `{T}`, losing the source, an `{X}` of mana or counters. A part that counts X
/// takes the X the caster announces.
pub(crate) fn variants(card: &CardDefinition) -> Option<Vec<Variant>> {
    let mut found = vec![Variant {
        mana: ManaCost::ZERO,
        parts: Vec::new(),
        words: Vec::new(),
    }];
    for rule in ability_reader::printed_rules(card) {
        let Ability::AdditionalCost {
            cost,
            required: true,
        } = &**rule
        else {
            continue;
        };
        let options = alternatives(cost);
        let mut next = Vec::new();
        for before in &found {
            for option in &options {
                let (mana, parts) = ways::cast_cost_counting(option, true)?;
                let mut words = before.words.clone();
                if options.len() > 1 {
                    words.push(words_of(&mana, &parts));
                }
                next.push(Variant {
                    mana: mana
                        .symbols()
                        .iter()
                        .copied()
                        .fold(before.mana.clone(), ManaCost::with),
                    parts: before.parts.iter().cloned().chain(parts).collect(),
                    words,
                });
            }
        }
        found = next;
    }
    u8::try_from(found.len()).is_ok().then_some(found)
}

/// A cost in words, lower case: its mana, then its parts.
pub(crate) fn words_of(mana: &ManaCost, parts: &[Part]) -> String {
    let mut words: Vec<String> = Vec::new();
    if mana.mana_value() > 0 {
        words.push(format!("pay {mana}"));
    }
    words.extend(parts.iter().map(|part| {
        let text = part.text();
        let mut chars = text.chars();
        chars.next().map_or(String::new(), |first| {
            first.to_lowercase().chain(chars).collect()
        })
    }));
    words.join(" and ")
}

/// Whether a required additional cost of the card counts the X the caster announces ("pay X life", "sacrifice X creatures").
pub(crate) fn counts_x(card: &CardDefinition) -> bool {
    variants(card).is_some_and(|all| {
        all.iter()
            .any(|variant| variant.parts.iter().any(Part::is_counted))
    })
}

/// `ways` with the required additional costs of the card added to each, one way for each cost the caster may choose; none
/// when the caster cannot be asked to pay them.
pub(crate) fn add_to(ways: Vec<Way>, card: &CardDefinition) -> Vec<Way> {
    let Some(all) = variants(card) else {
        return Vec::new();
    };
    if all.len() == 1 && all[0].parts.is_empty() && all[0].mana == ManaCost::ZERO {
        return ways;
    }
    let printed = card.mana_cost.as_ref();
    let mut made = Vec::new();
    for way in ways {
        for (at, variant) in (0u8..).zip(&all) {
            let mut chosen = way.clone();
            chosen.parts.extend(variant.parts.iter().cloned());
            if variant.mana != ManaCost::ZERO {
                let base = way.cost(printed).unwrap_or(ManaCost::ZERO);
                chosen.mana = Some(
                    variant
                        .mana
                        .symbols()
                        .iter()
                        .copied()
                        .fold(base, ManaCost::with),
                );
            }
            if !variant.words.is_empty() {
                chosen.label = format!("{}, {}", way.label, variant.words.join(", "));
            }
            // A way that is a variant of its own (harmonize with a creature tapped) keeps that mark.
            chosen.variant = way.variant | at;
            made.push(chosen);
        }
    }
    made
}

#[cfg(test)]
mod tests {
    //! Goal: the required additional costs of a card are the parts and the mana it prints in order, added to every way it is cast, one way for each cost it lets the caster choose, and a card with a cost the announcement does not pay has no way to be cast.
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
            label: "Cast for {1}{G}".to_owned(),
            mana: None,
            instant: false,
            variant: 0,
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

    fn choice(costs: Vec<Vec<CostItem>>) -> CostItem {
        CostItem::OneOf(costs.into_iter().map(Cost).collect())
    }

    fn parts_of(card: &CardDefinition) -> Option<Vec<usize>> {
        variants(card).map(|all| all.iter().map(|variant| variant.parts.len()).collect())
    }

    #[rstest]
    #[case::none(vec![], vec![0])]
    #[case::a_discard(vec![required(vec![discard()])], vec![1])]
    #[case::a_discard_and_a_sacrifice(vec![required(vec![discard(), sacrifice()])], vec![2])]
    #[case::two_abilities(vec![required(vec![discard()]), required(vec![sacrifice()])], vec![2])]
    #[case::life(vec![required(vec![CostItem::PayLife(Amount::Fixed(3))])], vec![1])]
    #[case::mana(vec![required(vec![CostItem::Mana("{2}".parse().unwrap())])], vec![0])]
    #[case::an_optional_one_is_not_required(
        vec![Ability::AdditionalCost { cost: Cost(vec![discard()]), required: false }],
        vec![0]
    )]
    #[case::a_choice_of_two(
        vec![required(vec![choice(vec![vec![discard()], vec![CostItem::PayLife(Amount::Fixed(3))]])])],
        vec![1, 1]
    )]
    #[case::a_choice_with_mana(
        vec![required(vec![choice(vec![vec![discard()], vec![CostItem::Mana("{2}".parse().unwrap())]])])],
        vec![1, 0]
    )]
    #[case::a_choice_of_three(
        vec![required(vec![choice(vec![vec![sacrifice()], vec![discard()], vec![CostItem::PayLife(Amount::Fixed(4))]])])],
        vec![1, 1, 1]
    )]
    #[case::two_choices_make_every_pair(
        vec![
            required(vec![choice(vec![vec![sacrifice()], vec![discard()]])]),
            required(vec![choice(vec![vec![CostItem::PayLife(Amount::Fixed(1))], vec![CostItem::PayLife(Amount::Fixed(2))], vec![discard()]])]),
        ],
        vec![2; 6]
    )]
    #[case::a_choice_beside_a_plain_cost(
        vec![
            required(vec![sacrifice()]),
            required(vec![choice(vec![vec![discard()], vec![CostItem::PayLife(Amount::Fixed(3))]])]),
        ],
        vec![2, 2]
    )]
    fn the_variants_are_the_costs_in_print_order_one_for_each_choice(
        #[case] rules: Vec<Ability>,
        #[case] count: Vec<usize>,
    ) {
        assert_eq!(parts_of(&card(rules)), Some(count));
    }

    #[rstest]
    #[case::tap(required(vec![CostItem::Tap]))]
    #[case::the_source(required(vec![CostItem::SacrificeSelf]))]
    #[case::x_mana(required(vec![CostItem::Mana("{X}".parse().unwrap())]))]
    #[case::a_choice_with_a_tap(required(vec![choice(vec![vec![discard()], vec![CostItem::Tap]])]))]
    fn a_cost_the_announcement_does_not_pay_leaves_no_way_to_cast(#[case] rule: Ability) {
        let card = card(vec![rule]);
        assert_eq!(variants(&card), None);
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
        let read = variants(&card).expect("the cost is paid by the announcement");
        assert_eq!(read[0].parts.len(), 1);
        assert_eq!(counts_x(&card), counts);
        assert_eq!(read[0].parts[0].is_counted(), counts);
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
    fn a_choice_makes_a_way_for_each_cost_and_each_way() {
        let rules = card(vec![required(vec![choice(vec![
            vec![discard()],
            vec![CostItem::PayLife(Amount::Fixed(3))],
        ])])]);
        let ways = add_to(vec![way(Vec::new()), way(vec![Part::Life(1)])], &rules);
        let seen: Vec<(u8, usize)> = ways
            .iter()
            .map(|made| (made.variant, made.parts.len()))
            .collect();
        assert_eq!(seen, vec![(0, 1), (1, 1), (0, 2), (1, 2)]);
        assert_eq!(ways[0].label, "Cast for {1}{G}, discard a card");
        assert_eq!(ways[1].label, "Cast for {1}{G}, pay 3 life");
    }

    #[rstest]
    #[case::the_printed_cost(None, "{3}{G}")]
    #[case::an_alternative_cost(Some("{R}"), "{2}{R}")]
    fn mana_in_a_required_cost_joins_the_mana_of_the_way(
        #[case] own: Option<&str>,
        #[case] total: &str,
    ) {
        let rules = card(vec![required(vec![CostItem::Mana("{2}".parse().unwrap())])]);
        let mut base = way(Vec::new());
        base.mana = own.map(|text| ManaCost::parse(text).unwrap());
        let mut with_printed = rules;
        with_printed.mana_cost = Some(ManaCost::parse("{1}{G}").unwrap());
        let made = add_to(vec![base], &with_printed);
        let total = ManaCost::parse(total).unwrap();
        // The printed cost {1}{G} plus {2} is {3}{G}; an alternative cost {R} plus {2} is {2}{R}.
        assert_eq!(
            made[0].mana.as_ref().map(ManaCost::mana_value),
            Some(total.mana_value())
        );
    }

    #[test]
    fn a_card_with_no_required_cost_keeps_its_ways() {
        let ways = vec![way(Vec::new()), way(vec![Part::Life(1)])];
        assert_eq!(add_to(ways.clone(), &card(Vec::new())), ways);
    }
}
