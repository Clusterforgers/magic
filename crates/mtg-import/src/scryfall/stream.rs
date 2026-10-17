//! Iterators that stream normalized records out of Scryfall bulk files.

use std::io::Read;
use std::marker::PhantomData;

use thiserror::Error;

use super::array_stream::{ArrayStream, StreamError};
use super::normalize::InvalidRecord;
use super::raw::{RawCard, RawRuling};
use super::record::{CardRecord, RulingRecord};

/// Failure while reading a bulk file; only [`ScryfallError::Stream`] ends the iteration.
#[derive(Debug, Error)]
pub enum ScryfallError {
    /// The array framing is broken or the reader failed; no further records follow.
    #[error("bulk data is malformed: {0}")]
    Stream(#[from] StreamError),
    /// One entry is not valid JSON for its type; later entries are still read.
    #[error("entry {index} is not valid: {source}")]
    Json {
        /// Zero-based position of the entry in the array.
        index: usize,
        /// The underlying JSON error.
        source: serde_json::Error,
    },
    /// One entry parsed but lacks required data; later entries are still read.
    #[error("entry {index} is unusable: {reason}")]
    Invalid {
        /// Zero-based position of the entry in the array.
        index: usize,
        /// What is missing.
        reason: InvalidRecord,
    },
}

impl ScryfallError {
    /// Whether the stream stops after this error.
    pub fn is_fatal(&self) -> bool {
        matches!(self, ScryfallError::Stream(_))
    }
}

/// A record type that can be built from one bulk-file array element.
pub trait BulkRecord: Sized {
    /// Parses and normalizes one element's JSON bytes.
    fn from_element(bytes: &[u8]) -> Result<Self, ElementError>;
}

/// Why a single element could not become a record.
#[derive(Debug, Error)]
pub enum ElementError {
    /// The bytes are not valid JSON for the raw type.
    #[error(transparent)]
    Json(serde_json::Error),
    /// The raw object lacks required data.
    #[error(transparent)]
    Invalid(InvalidRecord),
}

impl BulkRecord for CardRecord {
    fn from_element(bytes: &[u8]) -> Result<Self, ElementError> {
        let raw: RawCard = serde_json::from_slice(bytes).map_err(ElementError::Json)?;
        CardRecord::try_from(raw).map_err(ElementError::Invalid)
    }
}

impl BulkRecord for RulingRecord {
    fn from_element(bytes: &[u8]) -> Result<Self, ElementError> {
        let raw: RawRuling = serde_json::from_slice(bytes).map_err(ElementError::Json)?;
        RulingRecord::try_from(raw).map_err(ElementError::Invalid)
    }
}

/// Lazily yields records from a bulk array, holding one element in memory at a time.
pub struct BulkStream<R: Read, T> {
    elements: ArrayStream<R>,
    index: usize,
    record: PhantomData<fn() -> T>,
}

/// Stream of cards from Oracle Cards or Default Cards bulk data.
pub type CardStream<R> = BulkStream<R, CardRecord>;
/// Stream of rulings from the rulings bulk file.
pub type RulingStream<R> = BulkStream<R, RulingRecord>;

impl<R: Read, T: BulkRecord> BulkStream<R, T> {
    /// Streams records from `reader`, which must yield a JSON array of objects.
    pub fn new(reader: R) -> Self {
        Self {
            elements: ArrayStream::new(reader),
            index: 0,
            record: PhantomData,
        }
    }
}

impl<R: Read, T: BulkRecord> Iterator for BulkStream<R, T> {
    type Item = Result<T, ScryfallError>;

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = match self.elements.next_element() {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return None,
            Err(e) => return Some(Err(e.into())),
        };
        let index = self.index;
        self.index += 1;
        Some(T::from_element(&bytes).map_err(|e| match e {
            ElementError::Json(source) => ScryfallError::Json { index, source },
            ElementError::Invalid(reason) => ScryfallError::Invalid { index, reason },
        }))
    }
}

/// Streams cards from Oracle Cards or Default Cards bulk JSON.
pub fn cards<R: Read>(reader: R) -> CardStream<R> {
    BulkStream::new(reader)
}

/// Streams rulings from the rulings bulk JSON.
pub fn rulings<R: Read>(reader: R) -> RulingStream<R> {
    BulkStream::new(reader)
}

#[cfg(test)]
mod tests {
    //! Goal: bulk streams isolate bad entries, stop only on broken framing, and read lazily so huge files never sit in memory.
    use std::cell::Cell;
    use std::io;
    use std::rc::Rc;

    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn card_json(n: usize) -> String {
        format!(
            r#"{{"id":"p{n}","oracle_id":"o{n}","name":"Card {n}","cmc":{n}.0,"type_line":"Instant","colors":["R"]}}"#
        )
    }

    fn array(items: &[String]) -> String {
        format!("[{}]", items.join(","))
    }

    #[test]
    fn yields_records_in_file_order() {
        let input = array(&(0..5).map(card_json).collect::<Vec<_>>());
        let names: Vec<String> = cards(input.as_bytes()).map(|c| c.unwrap().name).collect();
        assert_eq!(names, ["Card 0", "Card 1", "Card 2", "Card 3", "Card 4"]);
    }

    #[test]
    fn bad_entries_are_reported_with_their_index_and_do_not_stop_the_stream() {
        let items = [
            card_json(0),
            r#"{"id":"p","oracle_id":"o","cmc":"not a number","name":"X"}"#.to_owned(),
            r#"{"id":"p","oracle_id":"o"}"#.to_owned(),
            r#"{"id":"p","oracle_id":"o","name":"Y","card_faces":[{"mana_cost":"{1}"}]}"#
                .to_owned(),
            card_json(4),
        ];
        let results: Vec<_> = cards(array(&items).as_bytes()).collect();
        assert_eq!(results.len(), 5);
        assert!(results[0].is_ok() && results[4].is_ok());
        assert!(matches!(
            results[1],
            Err(ScryfallError::Json { index: 1, .. })
        ));
        assert!(matches!(
            results[2],
            Err(ScryfallError::Invalid {
                index: 2,
                reason: InvalidRecord::MissingName
            })
        ));
        assert!(matches!(
            results[3],
            Err(ScryfallError::Invalid {
                index: 3,
                reason: InvalidRecord::MissingFaceName { .. }
            })
        ));
        assert_eq!(results.iter().flatten().count(), 2);
        assert!(
            results
                .iter()
                .filter_map(|r| r.as_ref().err())
                .all(|e| !e.is_fatal())
        );
    }

    #[rstest]
    #[case::truncated("[{\"id\":\"p\"")]
    #[case::not_an_array("{\"id\":\"p\"}")]
    #[case::empty("")]
    #[case::trailing_comma("[{\"id\":\"p\",\"oracle_id\":\"o\",\"name\":\"N\"},]")]
    #[case::garbage("\u{1}\u{2}\u{3}")]
    fn broken_framing_is_fatal_and_ends_iteration(#[case] input: &str) {
        let mut stream = cards(input.as_bytes());
        let mut saw_fatal = false;
        for item in stream.by_ref() {
            if let Err(e) = item {
                assert!(e.is_fatal(), "{e}");
                saw_fatal = true;
                break;
            }
        }
        assert!(saw_fatal);
        assert!(stream.next().is_none());
    }

    #[test]
    fn empty_array_yields_nothing() {
        assert_eq!(cards("[]".as_bytes()).count(), 0);
        assert_eq!(rulings(" [ ] ".as_bytes()).count(), 0);
    }

    #[test]
    fn invalid_utf8_inside_an_entry_only_spoils_that_entry() {
        let mut bytes = b"[{\"id\":\"p\",\"oracle_id\":\"o\",\"name\":\"Bad \xff\xfe\"},".to_vec();
        bytes.extend_from_slice(card_json(1).as_bytes());
        bytes.push(b']');
        let results: Vec<_> = cards(bytes.as_slice()).collect();
        assert!(matches!(
            results[0],
            Err(ScryfallError::Json { index: 0, .. })
        ));
        assert!(results[1].is_ok());
    }

    #[test]
    fn rulings_stream_normalizes_and_isolates_bad_entries() {
        let input = r#"[
            {"oracle_id":"o","source":"wotc","published_at":"2020-01-01","comment":"ok"},
            {"oracle_id":"o","source":"wotc"},
            {"oracle_id":7,"comment":"wrong type"},
            {"oracle_id":"o2","source":"scryfall","comment":"also ok"}
        ]"#;
        let results: Vec<_> = rulings(input.as_bytes()).collect();
        assert_eq!(results.len(), 4);
        assert_eq!(
            results[0].as_ref().unwrap().source,
            super::super::RulingSource::Wotc
        );
        assert!(matches!(
            results[1],
            Err(ScryfallError::Invalid {
                index: 1,
                reason: InvalidRecord::RulingWithoutComment { .. }
            })
        ));
        assert!(matches!(
            results[2],
            Err(ScryfallError::Json { index: 2, .. })
        ));
        assert_eq!(results[3].as_ref().unwrap().oracle_id.as_str(), "o2");
    }

    /// Reader over an endless-looking card array that counts how many bytes were pulled.
    struct CountingReader {
        pending: Vec<u8>,
        next_card: usize,
        total_cards: usize,
        served: Rc<Cell<usize>>,
    }

    impl Read for CountingReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pending.is_empty() {
                if self.next_card == 0 {
                    self.pending.push(b'[');
                }
                if self.next_card < self.total_cards {
                    if self.next_card > 0 {
                        self.pending.push(b',');
                    }
                    self.pending
                        .extend_from_slice(card_json(self.next_card).as_bytes());
                    self.next_card += 1;
                } else if self.next_card == self.total_cards {
                    self.pending.push(b']');
                    self.next_card += 1;
                }
            }
            let n = buf.len().min(self.pending.len());
            buf[..n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            self.served.set(self.served.get() + n);
            Ok(n)
        }
    }

    #[test]
    fn reads_lazily_instead_of_loading_the_whole_input() {
        let total_cards = 200_000;
        let served = Rc::new(Cell::new(0));
        let reader = CountingReader {
            pending: Vec::new(),
            next_card: 0,
            total_cards,
            served: Rc::clone(&served),
        };
        let mut stream = cards(reader);
        for _ in 0..10 {
            assert!(stream.next().unwrap().is_ok());
        }
        let approx_total = total_cards * card_json(100_000).len();
        assert!(
            served.get() * 100 < approx_total,
            "read {} of ~{approx_total} bytes after ten cards",
            served.get()
        );
    }

    proptest! {
        #[test]
        fn every_valid_entry_is_returned_exactly_once(
            ok_flags in prop::collection::vec(any::<bool>(), 0..40),
        ) {
            let items: Vec<String> = ok_flags
                .iter()
                .enumerate()
                .map(|(n, ok)| if *ok { card_json(n) } else { format!(r#"{{"id":"p{n}"}}"#) })
                .collect();
            let results: Vec<_> = cards(array(&items).as_bytes()).collect();
            prop_assert_eq!(results.len(), ok_flags.len());
            for (n, (result, ok)) in results.iter().zip(&ok_flags).enumerate() {
                prop_assert_eq!(result.is_ok(), *ok);
                if let Ok(card) = result {
                    prop_assert_eq!(card.name.clone(), format!("Card {n}"));
                }
            }
        }
    }
}
