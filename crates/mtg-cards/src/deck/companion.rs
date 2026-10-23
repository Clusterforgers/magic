//! The deck check of a companion (CR 702.139a): the starting deck, the main deck and the commanders, meets the
//! condition of the companion ability the rules library holds for the card in the companion zone.

use std::collections::BTreeMap;

use crate::ability::Ability;
use crate::companion::DeckCard;
use crate::keyword::Keyword;
use crate::load::Library;
use crate::support::Caps;

use super::legality::{Issue, IssueCode};
use super::{CardFacts, DeckZone, name_of};

/// Cards a Commander deck holds, its commanders included.
const COMMANDER_MINIMUM: usize = 100;
/// Cards a deck of any other format holds at least.
const OTHER_MINIMUM: usize = 60;

/// The issue of the companion of a deck, if there is one and its condition is not met. A companion whose card has no
/// rules the library reads is not judged: the game runs no companion for it, so the card is a manual one.
pub(super) fn issues(
    format: &str,
    deck: &BTreeMap<(DeckZone, String), u32>,
    cards: &BTreeMap<String, CardFacts>,
    library: &Library,
    caps: &Caps,
) -> Vec<Issue> {
    let mut companions = deck
        .iter()
        .filter(|((zone, _), _)| *zone == DeckZone::Companion);
    let (Some((id, 1)), None) = (
        companions.next().map(|((_, id), count)| (id, *count)),
        companions.next(),
    ) else {
        return Vec::new();
    };
    let Some(condition) = library.card(id).and_then(|rules| {
        rules.abilities().find_map(|(_, ability)| match ability {
            Ability::Keyword(Keyword::Companion(condition)) => Some(condition),
            _ => None,
        })
    }) else {
        return Vec::new();
    };
    let mut starting: Vec<DeckCard> = Vec::new();
    for ((zone, card), count) in deck {
        if !matches!(zone, DeckZone::Main | DeckZone::Commander) {
            continue;
        }
        let Some(facts) = cards.get(card) else {
            continue;
        };
        let definition = library.assemble(&facts.printed, caps).definition;
        starting.extend(std::iter::repeat_n(
            DeckCard::from(&definition),
            *count as usize,
        ));
    }
    let minimum = if format == "commander" {
        COMMANDER_MINIMUM
    } else {
        OTHER_MINIMUM
    };
    match condition.check(&starting, minimum) {
        Ok(()) => Vec::new(),
        Err(reason) => vec![Issue::card(
            IssueCode::CompanionCondition,
            DeckZone::Companion,
            id,
            cards.get(id),
            format!(
                "{} cannot be the companion: {reason}",
                name_of(cards.get(id), id)
            ),
        )],
    }
}
