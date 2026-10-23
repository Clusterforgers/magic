//! Companion (CR 702.139): the condition a starting deck must meet for a card to be its companion, and the check of a
//! deck against it. The engine's seat check and the deck report both read a deck as a list of [`DeckCard`]s.

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::ColorSet;
use serde::{Deserialize, Serialize};

use crate::definition::CardDefinition;
use crate::refs::{Pred, Selector};
use crate::value::{Amount, StatKind};

/// What a companion asks of the starting deck.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum DeckCondition {
    /// Every card that matches `of` also matches `has`: "Each permanent card in your starting deck has mana value 2 or
    /// less" is `of` the permanent types and `has` a mana value of at most 2; "contains only cards with mana value 3
    /// or greater and land cards" is every card (`of` empty) that is a land or has a mana value of at least 3.
    Each {
        /// The cards the condition speaks of; every card of the deck when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        of: Selector,
        /// What each of them must be.
        has: Selector,
    },
    /// No two cards that match `of` share a name: "Each nonland card in your starting deck has a different name."
    Distinct {
        /// The cards that must differ; every card of the deck when empty.
        #[serde(default, skip_serializing_if = "Selector::is_empty")]
        of: Selector,
    },
    /// The deck holds at least this many cards more than the format's minimum deck size.
    ExtraCards(u32),
}

/// A card of a starting deck as the condition reads it: the characteristics the predicates ask about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckCard {
    /// The card's name.
    pub name: String,
    /// Its mana value.
    pub mana_value: u32,
    /// Its card types.
    pub types: BTreeSet<CardType>,
    /// Its supertypes.
    pub supertypes: BTreeSet<Supertype>,
    /// Its subtypes.
    pub subtypes: Vec<Subtype>,
    /// Its colours.
    pub colors: ColorSet,
}

impl From<&CardDefinition> for DeckCard {
    fn from(card: &CardDefinition) -> Self {
        DeckCard {
            name: card.name.clone(),
            mana_value: card.mana_value(),
            types: card.types.clone(),
            supertypes: card.supertypes.clone(),
            subtypes: card.subtypes.clone(),
            colors: card.colors(),
        }
    }
}

/// Whether the card matches the predicate; `None` for a predicate that speaks of anything but the card itself.
fn matches(pred: &Pred, card: &DeckCard) -> Option<bool> {
    Some(match pred {
        Pred::Type(ty) => card.types.contains(ty),
        Pred::Supertype(supertype) => card.supertypes.contains(supertype),
        Pred::Subtype(subtype) => card.subtypes.contains(subtype),
        Pred::Color(color) => card.colors.contains(*color),
        Pred::Colorless => card.colors == ColorSet::EMPTY,
        Pred::Named(name) => card.name == *name,
        Pred::Compare {
            stat: StatKind::ManaValue,
            op,
            to: Amount::Fixed(to),
        } => op.holds(i64::from(card.mana_value), i64::from(*to)),
        Pred::Not(inner) => !matches(inner, card)?,
        Pred::AnyOf(any) => {
            let mut found = false;
            for inner in any {
                found |= matches(inner, card)?;
            }
            found
        }
        _ => return None,
    })
}

/// Whether the card matches every predicate of the selector; a predicate about anything but the card counts as unmet.
fn selects(selector: &Selector, card: &DeckCard) -> bool {
    selector
        .0
        .iter()
        .all(|pred| matches(pred, card).unwrap_or(false))
}

impl DeckCondition {
    /// The predicates the condition cannot read from a card alone, as words; empty for a condition that can be checked.
    pub fn unreadable(&self) -> Vec<String> {
        let selectors: Vec<&Selector> = match self {
            DeckCondition::Each { of, has } => vec![of, has],
            DeckCondition::Distinct { of } => vec![of],
            DeckCondition::ExtraCards(_) => Vec::new(),
        };
        let probe = DeckCard {
            name: String::new(),
            mana_value: 0,
            types: BTreeSet::new(),
            supertypes: BTreeSet::new(),
            subtypes: Vec::new(),
            colors: ColorSet::EMPTY,
        };
        selectors
            .into_iter()
            .flat_map(|selector| &selector.0)
            .filter(|pred| matches(pred, &probe).is_none())
            .map(|pred| format!("{pred:?}"))
            .collect()
    }

    /// Checks the starting deck (all its cards, commanders included) against the condition, given the format's
    /// minimum deck size: `Ok` or the reason it fails, naming a card where one breaks it.
    pub fn check(&self, deck: &[DeckCard], minimum: usize) -> Result<(), String> {
        match self {
            DeckCondition::Each { of, has } => {
                match deck
                    .iter()
                    .find(|card| selects(of, card) && !selects(has, card))
                {
                    Some(card) => Err(format!(
                        "{} does not fit the companion's condition",
                        card.name
                    )),
                    None => Ok(()),
                }
            }
            DeckCondition::Distinct { of } => {
                let mut seen: BTreeMap<&str, u32> = BTreeMap::new();
                for card in deck.iter().filter(|card| selects(of, card)) {
                    *seen.entry(card.name.as_str()).or_default() += 1;
                }
                match seen.into_iter().find(|(_, copies)| *copies > 1) {
                    Some((name, copies)) => Err(format!(
                        "{name} is in the deck {copies} times, and the companion wants every card to differ"
                    )),
                    None => Ok(()),
                }
            }
            DeckCondition::ExtraCards(extra) => {
                let wanted = minimum + usize::try_from(*extra).unwrap_or(usize::MAX - minimum);
                if deck.len() >= wanted {
                    Ok(())
                } else {
                    Err(format!(
                        "the deck has {} cards and the companion wants at least {wanted}",
                        deck.len()
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a companion condition holds for a deck exactly when every card it speaks of fits, whatever the kinds of card in the deck; the conditions of the printed companions that read from the card alone, a distinct-names rule and a deck size over the minimum are read, and a predicate about anything but the card is reported as unreadable.
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;
    use crate::refs::Pred;
    use crate::value::Cmp;

    fn card(name: &str, cost: &str, types: &[CardType], subtypes: &[&str]) -> DeckCard {
        let mut definition = CardDefinition::new(name);
        for ty in types {
            definition = definition.with_type(*ty);
        }
        for subtype in subtypes {
            definition = definition.with_subtype(*subtype);
        }
        if !cost.is_empty() {
            definition = definition.with_mana_cost(ManaCost::parse(cost).unwrap());
        }
        DeckCard::from(&definition)
    }

    fn value(op: Cmp, to: i32) -> Pred {
        Pred::Compare {
            stat: StatKind::ManaValue,
            op,
            to: Amount::Fixed(to),
        }
    }

    fn permanent() -> Pred {
        Pred::AnyOf(vec![
            Pred::Type(CardType::Artifact),
            Pred::Type(CardType::Creature),
            Pred::Type(CardType::Enchantment),
            Pred::Type(CardType::Land),
            Pred::Type(CardType::Planeswalker),
            Pred::Type(CardType::Battle),
        ])
    }

    fn lurrus() -> DeckCondition {
        DeckCondition::Each {
            of: Selector(vec![permanent()]),
            has: Selector(vec![value(Cmp::Le, 2)]),
        }
    }

    fn keruga() -> DeckCondition {
        DeckCondition::Each {
            of: Selector::default(),
            has: Selector(vec![Pred::AnyOf(vec![
                Pred::Type(CardType::Land),
                value(Cmp::Ge, 3),
            ])]),
        }
    }

    fn kaheera() -> DeckCondition {
        DeckCondition::Each {
            of: Selector(vec![Pred::Type(CardType::Creature)]),
            has: Selector(vec![Pred::AnyOf(
                ["Cat", "Elemental", "Nightmare", "Dinosaur", "Beast"]
                    .map(|name| Pred::Subtype(Subtype(name.into())))
                    .to_vec(),
            )]),
        }
    }

    fn lutri() -> DeckCondition {
        DeckCondition::Distinct {
            of: Selector(vec![Pred::Not(Box::new(Pred::Type(CardType::Land)))]),
        }
    }

    #[rstest]
    #[case::lurrus_with_cheap_permanents(lurrus(), &[("A", "{1}{G}", CardType::Creature), ("B", "{W}", CardType::Artifact)], true)]
    #[case::lurrus_with_a_three_drop(lurrus(), &[("A", "{1}{G}", CardType::Creature), ("B", "{2}{W}", CardType::Creature)], false)]
    #[case::lurrus_ignores_an_expensive_sorcery(lurrus(), &[("A", "{4}{R}", CardType::Sorcery)], true)]
    #[case::lurrus_ignores_nothing_in_an_empty_deck(lurrus(), &[], true)]
    #[case::keruga_with_a_land_and_a_big_spell(keruga(), &[("A", "", CardType::Land), ("B", "{2}{G}", CardType::Creature)], true)]
    #[case::keruga_with_a_cheap_spell(keruga(), &[("A", "", CardType::Land), ("B", "{G}", CardType::Instant)], false)]
    #[case::lutri_with_two_lands_of_a_name(lutri(), &[("Forest", "", CardType::Land), ("Forest", "", CardType::Land), ("A", "{G}", CardType::Instant)], true)]
    #[case::lutri_with_a_repeated_spell(lutri(), &[("A", "{G}", CardType::Instant), ("A", "{G}", CardType::Instant)], false)]
    fn conditions_over_a_deck(
        #[case] condition: DeckCondition,
        #[case] cards: &[(&str, &str, CardType)],
        #[case] holds: bool,
    ) {
        let deck: Vec<DeckCard> = cards
            .iter()
            .map(|(name, cost, ty)| card(name, cost, &[*ty], &[]))
            .collect();
        assert_eq!(condition.check(&deck, 60).is_ok(), holds);
    }

    #[rstest]
    #[case::a_cat(&["Cat"], true)]
    #[case::a_beast_and_an_elemental(&["Beast", "Elemental"], true)]
    #[case::a_human(&["Human"], false)]
    #[case::a_cat_and_a_human(&["Cat", "Human"], false)]
    fn the_creature_types_of_kaheera(#[case] subtypes: &[&str], #[case] holds: bool) {
        let deck: Vec<DeckCard> = subtypes
            .iter()
            .map(|subtype| card("C", "{1}", &[CardType::Creature], &[subtype]))
            .collect();
        assert_eq!(kaheera().check(&deck, 60).is_ok(), holds);
    }

    #[rstest]
    #[case::exactly_twenty_over(80, 60, true)]
    #[case::one_short(79, 60, false)]
    #[case::the_minimum_alone(60, 60, false)]
    #[case::commander_deck_cannot(100, 100, false)]
    fn extra_cards_over_the_minimum(
        #[case] size: usize,
        #[case] minimum: usize,
        #[case] holds: bool,
    ) {
        let deck = vec![card("A", "{1}", &[CardType::Creature], &[]); size];
        assert_eq!(
            DeckCondition::ExtraCards(20).check(&deck, minimum).is_ok(),
            holds
        );
    }

    #[test]
    fn a_predicate_about_more_than_the_card_is_unreadable() {
        let condition = DeckCondition::Each {
            of: Selector(vec![Pred::Tapped, Pred::Type(CardType::Creature)]),
            has: Selector(vec![Pred::Token]),
        };
        assert_eq!(condition.unreadable().len(), 2);
        assert_eq!(lurrus().unreadable(), Vec::<String>::new());
        assert_eq!(lutri().unreadable(), Vec::<String>::new());
    }

    #[test]
    fn a_condition_round_trips_through_json_and_leaves_an_empty_selector_out() {
        for condition in [lurrus(), keruga(), lutri(), DeckCondition::ExtraCards(20)] {
            let json = serde_json::to_string(&condition).unwrap();
            assert_eq!(
                serde_json::from_str::<DeckCondition>(&json).unwrap(),
                condition
            );
        }
        assert_eq!(
            serde_json::to_string(&DeckCondition::Distinct {
                of: Selector::default()
            })
            .unwrap(),
            r#"{"Distinct":{}}"#
        );
    }
}
