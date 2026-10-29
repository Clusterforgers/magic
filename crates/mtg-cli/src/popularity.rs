//! The popularity of cards: the Scryfall `edhrec_rank` of each oracle id, read at run time from the cached Oracle Cards file.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::bulk::w2a_open_elements;
use crate::cache::Cache;
use crate::error::CliError;
use crate::w2a_promote::fixture::w2a_oracle_cards;

/// The weight of a card at `rank`: 1 / (rank + 100), so the most played cards count most.
pub fn weight_of(rank: u32) -> f64 {
    1.0 / (f64::from(rank) + 100.0)
}

/// The EDHREC rank of each card that has one (1 is the most played).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Popularity {
    ranks: BTreeMap<String, u32>,
    last: u32,
}

/// The part of an Oracle Cards record that carries the rank.
#[derive(Deserialize)]
struct Record {
    oracle_id: Option<String>,
    edhrec_rank: Option<u32>,
}

impl Popularity {
    /// The popularity the ranks give; `None` when no card has one.
    pub fn new(ranks: BTreeMap<String, u32>) -> Option<Self> {
        let last = ranks.values().copied().max()?;
        Some(Self { ranks, last })
    }

    /// The ranks of the Oracle Cards file the cache holds.
    pub fn from_cache(cache: &Cache) -> Result<Self, CliError> {
        Self::read(&w2a_oracle_cards(cache)?)
    }

    /// The ranks of an Oracle Cards file (gzip or plain, a JSON array or one record per line).
    pub fn read(path: &Path) -> Result<Self, CliError> {
        let broken = |error: &dyn std::fmt::Display| {
            CliError::Config(format!("{}: {error}", path.display()))
        };
        let mut elements = w2a_open_elements(path)?;
        let mut ranks = BTreeMap::new();
        while let Some(bytes) = elements.next_element().map_err(|e| broken(&e))? {
            let record: Record = serde_json::from_slice(&bytes).map_err(|e| broken(&e))?;
            if let Record {
                oracle_id: Some(id),
                edhrec_rank: Some(rank),
            } = record
            {
                ranks.insert(id, rank);
            }
        }
        Self::new(ranks).ok_or_else(|| {
            CliError::Config(format!(
                "{}: no card carries an edhrec_rank; `mtg-cli fetch scryfall --force` downloads a current file",
                path.display()
            ))
        })
    }

    /// The rank of a card, if it has one.
    pub fn rank(&self, oracle_id: &str) -> Option<u32> {
        self.ranks.get(oracle_id).copied()
    }

    /// The least played rank of the file: the rank a card without one is weighed at.
    pub fn last(&self) -> u32 {
        self.last
    }

    /// The weight of a card: that of its rank, or of the last rank when it has none.
    pub fn weight(&self, oracle_id: &str) -> f64 {
        weight_of(self.rank(oracle_id).unwrap_or(self.last))
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the weight is 1 / (rank + 100) and a card without a rank weighs as the last rank of the file; a file of any framing yields the ranks of the records that have one and no others, and a file with no rank is refused.
    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::the_most_played(1, 1.0 / 101.0)]
    #[case::a_hundred(100, 0.005)]
    #[case::two_thousand(2_000, 1.0 / 2_100.0)]
    #[case::the_tail(30_000, 1.0 / 30_100.0)]
    fn the_weight_is_the_inverse_of_the_rank_plus_a_hundred(#[case] rank: u32, #[case] want: f64) {
        assert!((weight_of(rank) - want).abs() < 1e-12);
    }

    fn popularity() -> Popularity {
        let ranks = [("a", 1), ("b", 250), ("c", 40)];
        Popularity::new(
            ranks
                .into_iter()
                .map(|(id, n)| (id.to_owned(), n))
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn a_card_without_a_rank_weighs_as_the_last_rank() {
        let popularity = popularity();
        assert_eq!(popularity.last(), 250);
        assert_eq!(popularity.rank("c"), Some(40));
        assert_eq!(popularity.rank("z"), None);
        assert!((popularity.weight("c") - weight_of(40)).abs() < 1e-12);
        assert!((popularity.weight("z") - weight_of(250)).abs() < 1e-12);
    }

    const RECORDS: [&str; 4] = [
        r#"{"oracle_id":"a","name":"A","edhrec_rank":7,"cmc":1.0}"#,
        r#"{"oracle_id":"b","name":"B","edhrec_rank":null}"#,
        r#"{"name":"No id","edhrec_rank":3}"#,
        r#"{"oracle_id":"d","name":"D"}"#,
    ];

    fn lines() -> Vec<u8> {
        RECORDS.join("\n").into_bytes()
    }

    fn array() -> Vec<u8> {
        format!("[{}]", RECORDS.join(",")).into_bytes()
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[rstest]
    #[case::lines(lines())]
    #[case::an_array(array())]
    #[case::gzipped_lines(gzip(&lines()))]
    #[case::a_gzipped_array(gzip(&array()))]
    fn only_the_records_with_an_id_and_a_rank_are_read(#[case] bytes: Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oracle.json");
        std::fs::write(&path, bytes).unwrap();

        let popularity = Popularity::read(&path).unwrap();

        assert_eq!(popularity.ranks, BTreeMap::from([("a".to_owned(), 7)]));
        assert_eq!(popularity.last(), 7);
    }

    #[test]
    fn a_file_without_any_rank_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oracle.json");
        std::fs::write(&path, r#"{"oracle_id":"a","edhrec_rank":null}"#).unwrap();

        let error = Popularity::read(&path).unwrap_err();

        assert!(matches!(error, CliError::Config(_)), "{error}");
        assert!(error.to_string().contains("edhrec_rank"), "{error}");
    }

    #[test]
    fn a_broken_record_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oracle.json");
        std::fs::write(&path, "{\"oracle_id\":\"a\",\"edhrec_rank\":\"high\"}\n").unwrap();

        let error = Popularity::read(&path).unwrap_err();

        assert!(error.to_string().contains("oracle.json"), "{error}");
    }
}
