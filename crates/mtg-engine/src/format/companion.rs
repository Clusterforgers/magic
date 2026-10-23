//! The deck rule of a companion (CR 702.139a): the card carries a companion ability and the starting deck, the
//! commanders included, meets its condition.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::companion::DeckCard;

use super::DeckError;

/// Checks that `companion` has a companion ability the engine reads and that the starting deck (the `deck` and the
/// `commanders`) meets its condition, given the format's `minimum` deck size.
pub(super) fn check_companion(
    companion: &CardDefinition,
    deck: &[Arc<CardDefinition>],
    commanders: &[Arc<CardDefinition>],
    minimum: usize,
) -> Result<(), DeckError> {
    let Some(condition) = companion.companion_condition() else {
        return Err(DeckError::NotACompanion {
            name: companion.name.clone(),
        });
    };
    let cards: Vec<DeckCard> = commanders
        .iter()
        .chain(deck)
        .map(|card| DeckCard::from(&**card))
        .collect();
    condition
        .check(&cards, minimum)
        .map_err(|reason| DeckError::CompanionCondition {
            name: companion.name.clone(),
            reason,
        })
}

#[cfg(test)]
mod tests {
    //! Goal: a companion is accepted when its card has the ability and the starting deck, commanders included, meets the condition, and refused with a typed error otherwise, in Normal at its minimum of 60 cards.
    use mtg_cards::companion::DeckCondition;
    use mtg_cards::keyword::Keyword;
    use mtg_cards::refs::{Pred, Selector};
    use mtg_core::card::CardType;
    use mtg_core::mana::ManaCost;
    use rstest::rstest;

    use super::*;

    fn companion(condition: Option<DeckCondition>) -> CardDefinition {
        let card = CardDefinition::new("Fixture Companion").with_type(CardType::Creature);
        match condition {
            Some(condition) => card.with_rules(mtg_cards::ability::Ability::Keyword(
                Keyword::companion(condition),
            )),
            None => card,
        }
    }

    fn card(name: &str, cost: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_mana_cost(ManaCost::parse(cost).unwrap()),
        )
    }

    fn cheap() -> DeckCondition {
        DeckCondition::Each {
            of: Selector::default(),
            has: Selector(vec![Pred::Compare {
                stat: mtg_cards::value::StatKind::ManaValue,
                op: mtg_cards::value::Cmp::Le,
                to: mtg_cards::value::Amount::Fixed(2),
            }]),
        }
    }

    #[rstest]
    #[case::fits(Some(cheap()), &["{G}", "{1}{G}"], &[], true)]
    #[case::a_commander_breaks_it(Some(cheap()), &["{G}"], &["{3}{G}"], false)]
    #[case::a_commander_fits(Some(cheap()), &["{G}"], &["{1}{G}"], true)]
    #[case::the_deck_breaks_it(Some(cheap()), &["{G}", "{2}{G}"], &[], false)]
    #[case::no_ability(None, &["{G}"], &[], false)]
    fn the_condition_is_read_over_the_whole_starting_deck(
        #[case] condition: Option<DeckCondition>,
        #[case] deck: &[&str],
        #[case] commanders: &[&str],
        #[case] accepted: bool,
    ) {
        let cards: Vec<_> = deck.iter().map(|cost| card("Spell", cost)).collect();
        let leaders: Vec<_> = commanders.iter().map(|cost| card("Leader", cost)).collect();
        let result = check_companion(&companion(condition.clone()), &cards, &leaders, 60);
        assert_eq!(result.is_ok(), accepted, "{result:?}");
        if !accepted {
            let error = result.unwrap_err();
            assert!(match condition {
                Some(_) => matches!(error, DeckError::CompanionCondition { .. }),
                None => matches!(error, DeckError::NotACompanion { .. }),
            });
        }
    }
}
