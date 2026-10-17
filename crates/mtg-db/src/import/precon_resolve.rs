//! Matching precon deck entries to stored cards by oracle id, printing id or name.

use std::collections::{HashMap, HashSet};

use mtg_import::mtgjson::PreconCard;
use mtg_import::resolve::normalize_name;
use serde::Serialize;
use sqlx::PgConnection;
use uuid::Uuid;

/// Why a deck entry could not be tied to one stored card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    /// No stored card has this id or name.
    NotFound,
    /// Several stored cards share the name and no id settled it.
    Ambiguous,
}

/// Stored cards that deck entries may refer to.
#[derive(Debug, Default)]
pub(crate) struct CardLookups {
    known: HashSet<Uuid>,
    by_printing: HashMap<Uuid, Uuid>,
    by_name: HashMap<String, Vec<Uuid>>,
    by_front_name: HashMap<String, Vec<Uuid>>,
}

impl CardLookups {
    /// Assembles lookups from stored `(oracle id, normalized name, normalized front name)` rows.
    pub(crate) fn from_parts(
        known: impl IntoIterator<Item = Uuid>,
        by_printing: impl IntoIterator<Item = (Uuid, Uuid)>,
        names: impl IntoIterator<Item = (Uuid, String, String)>,
    ) -> Self {
        let mut lookups = Self {
            known: known.into_iter().collect(),
            by_printing: by_printing.into_iter().collect(),
            ..Self::default()
        };
        for (oracle_id, full, front) in names {
            lookups.by_name.entry(full).or_default().push(oracle_id);
            lookups
                .by_front_name
                .entry(front)
                .or_default()
                .push(oracle_id);
        }
        lookups
    }

    /// Loads everything the given deck entries could match.
    pub(crate) async fn load(
        conn: &mut PgConnection,
        cards: &[&PreconCard],
    ) -> Result<Self, sqlx::Error> {
        let oracle_ids: Vec<Uuid> = cards
            .iter()
            .filter_map(|card| card.oracle_id.as_ref())
            .filter_map(|id| Uuid::parse_str(id.as_str()).ok())
            .collect();
        let printing_ids: Vec<Uuid> = cards
            .iter()
            .filter_map(|card| card.scryfall_id.as_ref())
            .filter_map(|id| Uuid::parse_str(id.as_str()).ok())
            .collect();
        let wanted_names: Vec<String> = cards
            .iter()
            .map(|card| normalize_name(&card.name))
            .collect();

        let known = sqlx::query_scalar!(
            "SELECT oracle_id FROM cards WHERE oracle_id = ANY($1)",
            &oracle_ids
        )
        .fetch_all(&mut *conn)
        .await?;
        let printings = sqlx::query!(
            "SELECT printing_id, oracle_id FROM printings WHERE printing_id = ANY($1)",
            &printing_ids
        )
        .fetch_all(&mut *conn)
        .await?;
        let by_name = sqlx::query!(
            "SELECT oracle_id, name_normalized, front_name_normalized FROM cards
             WHERE is_deck_card AND (name_normalized = ANY($1) OR front_name_normalized = ANY($1))
             ORDER BY oracle_id",
            &wanted_names
        )
        .fetch_all(&mut *conn)
        .await?;

        Ok(Self::from_parts(
            known,
            printings.into_iter().map(|p| (p.printing_id, p.oracle_id)),
            by_name
                .into_iter()
                .map(|n| (n.oracle_id, n.name_normalized, n.front_name_normalized)),
        ))
    }

    /// Resolves an entry: known oracle id first, then known printing id, then the name.
    ///
    /// A name matches the full card name, falling back to the front face, and never guesses
    /// between several cards.
    pub(crate) fn resolve(&self, card: &PreconCard) -> Result<Uuid, UnresolvedReason> {
        let by_oracle = card
            .oracle_id
            .as_ref()
            .and_then(|id| Uuid::parse_str(id.as_str()).ok())
            .filter(|id| self.known.contains(id));
        if let Some(id) = by_oracle {
            return Ok(id);
        }
        let by_printing = card
            .scryfall_id
            .as_ref()
            .and_then(|id| Uuid::parse_str(id.as_str()).ok())
            .and_then(|id| self.by_printing.get(&id));
        if let Some(&id) = by_printing {
            return Ok(id);
        }
        let key = normalize_name(&card.name);
        for table in [&self.by_name, &self.by_front_name] {
            match table.get(&key).map(Vec::as_slice) {
                Some([only]) => return Ok(*only),
                Some([_, _, ..]) => return Err(UnresolvedReason::Ambiguous),
                _ => {}
            }
        }
        Err(UnresolvedReason::NotFound)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: deck entries resolve by the strongest identifier available, fall back to names only when unambiguous, and are reported rather than guessed otherwise.
    use mtg_import::ids::{OracleId, PrintingId};
    use rstest::rstest;

    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn lookups() -> CardLookups {
        CardLookups::from_parts(
            [id(1), id(2)],
            [(id(10), id(1))],
            [
                (id(1), "lightning bolt".into(), "lightning bolt".into()),
                (id(2), "fire // ice".into(), "fire".into()),
                (id(3), "twin".into(), "twin".into()),
                (id(4), "twin".into(), "twin".into()),
                (id(5), "dusk // dawn".into(), "dusk".into()),
                (id(6), "dusk // night".into(), "dusk".into()),
            ],
        )
    }

    fn entry(name: &str, oracle: Option<u128>, printing: Option<u128>) -> PreconCard {
        PreconCard {
            count: 1,
            uuid: "u".into(),
            name: name.into(),
            set_code: None,
            number: None,
            is_foil: false,
            scryfall_id: printing.map(|n| PrintingId(id(n).to_string())),
            oracle_id: oracle.map(|n| OracleId(id(n).to_string())),
        }
    }

    #[rstest]
    #[case::oracle_id_wins_over_name(entry("Something Else", Some(1), None), Ok(id(1)))]
    #[case::oracle_id_wins_over_printing(entry("x", Some(2), Some(10)), Ok(id(2)))]
    #[case::printing_when_oracle_id_unknown(entry("x", Some(99), Some(10)), Ok(id(1)))]
    #[case::exact_name(entry("Lightning Bolt", None, None), Ok(id(1)))]
    #[case::name_is_normalized(entry("  LIGHTNING   bolt ", None, None), Ok(id(1)))]
    #[case::split_by_full_name(entry("Fire // Ice", None, None), Ok(id(2)))]
    #[case::split_by_slash_variant(entry("Fire/Ice", None, None), Ok(id(2)))]
    #[case::front_face_fallback(entry("Fire", None, None), Ok(id(2)))]
    #[case::unknown_name(entry("Nonexistent Card", None, None), Err(UnresolvedReason::NotFound))]
    #[case::unknown_ids_and_name(
        entry("Nope", Some(99), Some(98)),
        Err(UnresolvedReason::NotFound)
    )]
    #[case::same_name_twice(entry("Twin", None, None), Err(UnresolvedReason::Ambiguous))]
    #[case::shared_front_face(entry("Dusk", None, None), Err(UnresolvedReason::Ambiguous))]
    #[case::full_name_beats_ambiguous_front(entry("Dusk // Dawn", None, None), Ok(id(5)))]
    fn entries_resolve_in_priority_order(
        #[case] card: PreconCard,
        #[case] expected: Result<Uuid, UnresolvedReason>,
    ) {
        assert_eq!(lookups().resolve(&card), expected);
    }

    #[test]
    fn malformed_ids_fall_through_to_the_name() {
        let mut card = entry("Lightning Bolt", None, None);
        card.oracle_id = Some(OracleId("not-a-uuid".into()));
        card.scryfall_id = Some(PrintingId("also-not".into()));
        assert_eq!(lookups().resolve(&card), Ok(id(1)));
    }
}
