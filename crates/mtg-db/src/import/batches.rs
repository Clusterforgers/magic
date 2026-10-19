//! Chunks a fallible record stream into batches, setting aside bad records.

use std::fmt::Display;
use std::iter::Fuse;

use mtg_import::scryfall::ScryfallError;
use serde::Serialize;

use crate::runs::without_nul;

/// How many skipped records are described in detail; the rest are only counted.
const MAX_SAMPLES: usize = 20;

/// A record that was not imported, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedRecord {
    /// Zero-based position of the record in the input stream.
    pub position: u64,
    /// Why the record was skipped.
    pub reason: String,
}

/// Count of skipped records plus a bounded sample of reasons.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SkipLog {
    /// Total number of skipped records.
    pub count: u64,
    /// The first few skipped records.
    pub samples: Vec<SkippedRecord>,
}

impl SkipLog {
    /// Notes a skipped record at `position`.
    pub(crate) fn record(&mut self, position: u64, reason: impl Display) {
        self.count += 1;
        if self.samples.len() < MAX_SAMPLES {
            // Reasons echo record text and end up in the run's JSON counters, which cannot hold NUL.
            self.samples.push(SkippedRecord {
                position,
                reason: without_nul(&reason.to_string()),
            });
        }
    }
}

/// Reads good records from a stream in fixed-size batches.
pub(crate) struct Batches<I: Iterator> {
    items: Fuse<I>,
    size: usize,
    position: u64,
}

impl<T, I: Iterator<Item = Result<T, ScryfallError>>> Batches<I> {
    /// Batches of up to `size` records (at least one) from `items`.
    pub(crate) fn new(items: I, size: usize) -> Self {
        Self {
            items: items.fuse(),
            size: size.max(1),
            position: 0,
        }
    }

    /// Next batch as `(position, record)` pairs; empty once the stream ends.
    ///
    /// Non-fatal errors go to `skips`; a fatal error aborts and drops the partial batch.
    pub(crate) fn next_batch(
        &mut self,
        skips: &mut SkipLog,
    ) -> Result<Vec<(u64, T)>, ScryfallError> {
        let mut batch = Vec::with_capacity(self.size);
        while batch.len() < self.size {
            let Some(item) = self.items.next() else { break };
            let position = self.position;
            self.position += 1;
            match item {
                Ok(record) => batch.push((position, record)),
                Err(error) if error.is_fatal() => return Err(error),
                Err(error) => skips.record(position, error),
            }
        }
        Ok(batch)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: batching keeps every good record once and in order, sets bad ones aside with their stream position, and treats broken framing as the end of the import.
    use std::io::Cursor;

    use mtg_import::scryfall::{CardRecord, cards};
    use rstest::rstest;

    use super::*;

    fn card(n: usize) -> String {
        format!(r#"{{"id":"p{n}","oracle_id":"o{n}","name":"Card {n}"}}"#)
    }

    fn stream(entries: &[String]) -> impl Iterator<Item = Result<CardRecord, ScryfallError>> {
        cards(Cursor::new(format!("[{}]", entries.join(",")).into_bytes()))
    }

    fn drain(size: usize, entries: &[String]) -> (Vec<Vec<u64>>, SkipLog) {
        let mut batches = Batches::new(stream(entries), size);
        let mut skips = SkipLog::default();
        let mut seen = Vec::new();
        loop {
            let batch = batches.next_batch(&mut skips).unwrap();
            if batch.is_empty() {
                return (seen, skips);
            }
            seen.push(batch.iter().map(|(position, _)| *position).collect());
        }
    }

    #[rstest]
    #[case(1, 5, vec![vec![0], vec![1], vec![2], vec![3], vec![4]])]
    #[case(2, 5, vec![vec![0, 1], vec![2, 3], vec![4]])]
    #[case(5, 5, vec![vec![0, 1, 2, 3, 4]])]
    #[case(500, 3, vec![vec![0, 1, 2]])]
    #[case(0, 2, vec![vec![0], vec![1]])]
    #[case(3, 0, vec![])]
    fn records_are_chunked_in_order(
        #[case] size: usize,
        #[case] count: usize,
        #[case] expected: Vec<Vec<u64>>,
    ) {
        let entries: Vec<String> = (0..count).map(card).collect();
        assert_eq!(drain(size, &entries).0, expected);
    }

    #[test]
    fn bad_records_are_skipped_but_keep_their_stream_positions() {
        let entries = vec![
            card(0),
            r#"{"id":"x"}"#.to_owned(),
            card(2),
            r#"{"id":"y","oracle_id":"o","cmc":"nope","name":"N"}"#.to_owned(),
            card(4),
        ];
        let (batches, skips) = drain(2, &entries);
        assert_eq!(batches, vec![vec![0, 2], vec![4]]);
        assert_eq!(skips.count, 2);
        let positions: Vec<u64> = skips.samples.iter().map(|s| s.position).collect();
        assert_eq!(positions, [1, 3]);
    }

    #[test]
    fn only_a_bounded_sample_of_skips_is_kept() {
        let bad = MAX_SAMPLES + 5;
        let entries: Vec<String> = (0..bad).map(|n| format!(r#"{{"id":"x{n}"}}"#)).collect();
        let (batches, skips) = drain(10, &entries);
        assert_eq!(batches.len(), 0);
        assert_eq!(skips.count, bad as u64);
        assert_eq!(skips.samples.len(), MAX_SAMPLES);
    }

    #[test]
    fn broken_framing_is_fatal_and_drops_the_partial_batch() {
        let json = format!("[{},{},{{\"id\":", card(0), card(1));
        let mut batches = Batches::new(cards(json.as_bytes()), 10);
        let error = batches.next_batch(&mut SkipLog::default()).unwrap_err();
        assert!(error.is_fatal());
        let after = batches.next_batch(&mut SkipLog::default()).unwrap();
        assert_eq!(after.len(), 0);
    }
}
