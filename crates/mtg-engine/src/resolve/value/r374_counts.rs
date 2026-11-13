//! The counts of an object's types, keyword values and costs (`StatKind::R374*`): "for each supertype, card type, and
//! subtype it has", "for each of its creature types", "for each point of bushido it has", "its total toxic value",
//! "different mana costs" and "different color pairs".

use std::collections::BTreeSet;

use mtg_cards::keyword::Keyword;
use mtg_cards::refs::Selector;
use mtg_cards::value::{Amount, StatKind};
use mtg_core::card::{CardType, Supertype};

use super::{ResolveError, Scope, Seen};
use crate::keywords;

/// The refusal for a count of types of an object with every creature type, whose number the engine does not hold.
pub(crate) const R374_EVERY_CREATURE_TYPE: &str = "Stat.R374Types.EveryCreatureType";

/// Every card type, to count those an object has.
const CARD_TYPES: [CardType; 9] = [
    CardType::Artifact,
    CardType::Battle,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Instant,
    CardType::Kindred,
    CardType::Land,
    CardType::Planeswalker,
    CardType::Sorcery,
];

/// Every supertype, to count those an object has.
const SUPERTYPES: [Supertype; 4] = [
    Supertype::Basic,
    Supertype::Legendary,
    Supertype::Snow,
    Supertype::World,
];

/// Whether the characteristic is one this file counts.
pub(super) const fn is_r374(stat: StatKind) -> bool {
    matches!(
        stat,
        StatKind::R374Types
            | StatKind::R374CreatureTypes
            | StatKind::R374Bushido
            | StatKind::R374Toxic
    )
}

/// The subtypes the object has, printed ones and those effects gave it, without those an effect took away; `None` for an
/// object with every creature type.
fn subtypes(seen: &Seen<'_>) -> Option<Vec<String>> {
    if seen.chars.all_creature_types && seen.chars.types.contains(CardType::Creature) {
        return None;
    }
    let printed = seen
        .card
        .map(|card| card.card().subtypes.clone())
        .unwrap_or_default();
    let mut found: BTreeSet<String> = BTreeSet::new();
    for subtype in printed.iter().chain(seen.chars.subtypes.iter()) {
        let held =
            seen.chars.subtypes.contains(subtype) || seen.chars.has_subtype(&printed, subtype);
        if held {
            found.insert(subtype.0.clone());
        }
    }
    Some(found.into_iter().collect())
}

impl Scope<'_> {
    /// One of the counts of this file for the object; `Err` for a count of types of an object with every creature type.
    pub(super) fn r374_stat(&self, seen: &Seen<'_>, stat: StatKind) -> Result<i64, ResolveError> {
        let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
        Ok(match stat {
            StatKind::R374Types => {
                let subtypes =
                    subtypes(seen).ok_or(ResolveError::Unsupported(R374_EVERY_CREATURE_TYPE))?;
                let card_types = CARD_TYPES
                    .iter()
                    .filter(|t| seen.chars.types.contains(**t))
                    .count();
                let printed = seen
                    .card
                    .map(|card| card.card().supertypes.clone())
                    .unwrap_or_default();
                let supertypes = SUPERTYPES
                    .iter()
                    .filter(|s| {
                        (seen.chars.supertypes.contains(s) || printed.contains(s))
                            && !seen.chars.supertypes_removed.contains(s)
                    })
                    .count();
                count(subtypes.len() + card_types + supertypes)
            }
            StatKind::R374CreatureTypes => {
                if !seen.chars.types.contains(CardType::Creature)
                    && !seen.chars.types.contains(CardType::Kindred)
                {
                    return Ok(0);
                }
                let subtypes =
                    subtypes(seen).ok_or(ResolveError::Unsupported(R374_EVERY_CREATURE_TYPE))?;
                count(
                    subtypes
                        .iter()
                        .filter(|name| mtg_core::card::Subtype((*name).clone()).is_creature_type())
                        .count(),
                )
            }
            StatKind::R374Bushido | StatKind::R374Toxic => {
                let value = |keyword: &Keyword| match (stat, keyword) {
                    (StatKind::R374Toxic, Keyword::Toxic(n)) => i64::from(*n),
                    (StatKind::R374Bushido, Keyword::Macro { name, amount })
                        if name.eq_ignore_ascii_case("bushido") =>
                    {
                        i64::from(*amount)
                    }
                    _ => 0,
                };
                match seen.keywords {
                    Some(held) => held.iter().map(value).sum(),
                    None => seen.card.map_or(0, |card| {
                        keywords::keywords_of(self.state, card.id())
                            .into_iter()
                            .map(value)
                            .sum()
                    }),
                }
            }
            _ => 0,
        })
    }

    /// An object with every creature type among those the amount counts the creature types of, which reaches any cap.
    fn r374_every_type(&self, inner: &Amount) -> Result<bool, ResolveError> {
        let Amount::Stat {
            of,
            stat: StatKind::R374CreatureTypes | StatKind::R374Types,
        } = inner
        else {
            return Ok(false);
        };
        Ok(self.objects(of)?.iter().any(|object| {
            self.seen(self.as_it_left(*object)).is_some_and(|seen| {
                seen.chars.all_creature_types && seen.chars.types.contains(CardType::Creature)
            })
        }))
    }

    /// "..., to a maximum of N": the amount, never more than the number.
    pub(super) fn r374_at_most(&self, inner: &Amount, most: u32) -> Result<i64, ResolveError> {
        if self.r374_every_type(inner)? {
            return Ok(i64::from(most));
        }
        Ok(self.amount(inner)?.min(i64::from(most)))
    }

    /// How many different mana costs or color combinations there are among the objects; `None` for another
    /// characteristic. A card without a mana cost has none, and a colorless object no colors.
    pub(super) fn r374_distinct(
        &self,
        of: &Selector,
        stat: StatKind,
    ) -> Result<Option<i64>, ResolveError> {
        let mut found = BTreeSet::new();
        match stat {
            StatKind::R374ManaCost => {
                for object in self.matching(of)? {
                    let cost = self
                        .seen(object)
                        .and_then(|seen| seen.card)
                        .filter(|card| card.face_down().is_none())
                        .and_then(|card| card.mana_cost().map(|cost| format!("{cost:?}")));
                    found.extend(cost);
                }
            }
            StatKind::R374ColorSet => {
                for object in self.matching(of)? {
                    if let Some(seen) = self.seen(object)
                        && !seen.chars.colors.is_empty()
                    {
                        found.insert(format!(
                            "{:?}",
                            seen.chars.colors.iter().collect::<Vec<_>>()
                        ));
                    }
                }
            }
            _ => return Ok(None),
        }
        Ok(Some(i64::try_from(found.len()).unwrap_or(i64::MAX)))
    }
}
