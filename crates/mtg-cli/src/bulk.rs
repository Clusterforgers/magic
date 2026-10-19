//! Reading cached files: gzip or plain, a JSON array or one JSON object per line.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::sync::mpsc;

use flate2::read::MultiGzDecoder;
use mtg_import::scryfall::{
    CardRecord, RulingRecord, ScryfallError, cards, cards_jsonl, rulings, rulings_jsonl,
};

use crate::error::CliError;
use crate::progress::Progress;

/// A stream of records that may be moved to another thread.
pub type Records<T> = Box<dyn Iterator<Item = Result<T, ScryfallError>> + Send>;

/// How a bulk file frames its records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Framing {
    Array,
    Lines,
}

/// Opens a cached file, undoing gzip when it has the magic number, and finds out how it is framed.
fn open(path: &Path) -> io::Result<(BufReader<Box<dyn Read + Send>>, Framing)> {
    let mut file = BufReader::new(File::open(path)?);
    let gzip = file.fill_buf()?.starts_with(&[0x1f, 0x8b]);
    let inner: Box<dyn Read + Send> = if gzip {
        Box::new(MultiGzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut reader = BufReader::with_capacity(64 * 1024, inner);
    let framing = loop {
        let buffered = reader.fill_buf()?;
        let Some(&first) = buffered.first() else {
            break Framing::Lines;
        };
        if buffered.starts_with(&[0xEF, 0xBB, 0xBF]) {
            reader.consume(3);
        } else if first.is_ascii_whitespace() {
            reader.consume(1);
        } else if first == b'[' {
            break Framing::Array;
        } else {
            break Framing::Lines;
        }
    };
    Ok((reader, framing))
}

/// The cards of a cached Scryfall file.
pub fn open_cards(path: &Path) -> Result<Records<CardRecord>, CliError> {
    let (reader, framing) = open(path).map_err(|e| CliError::io(path, e))?;
    Ok(match framing {
        Framing::Array => Box::new(cards(reader)),
        Framing::Lines => Box::new(cards_jsonl(reader)),
    })
}

/// The rulings of a cached Scryfall file.
pub fn open_rulings(path: &Path) -> Result<Records<RulingRecord>, CliError> {
    let (reader, framing) = open(path).map_err(|e| CliError::io(path, e))?;
    Ok(match framing {
        Framing::Array => Box::new(rulings(reader)),
        Framing::Lines => Box::new(rulings_jsonl(reader)),
    })
}

/// Passes records through and tells the person watching how many went by.
pub struct Counted<I> {
    inner: I,
    seen: u64,
    every: u64,
    label: &'static str,
    progress: Progress,
}

impl<I> Counted<I> {
    /// Wraps `inner`, writing a line for `label` after every `every` records.
    pub fn new(inner: I, label: &'static str, every: u64, progress: Progress) -> Self {
        Self {
            inner,
            seen: 0,
            every: every.max(1),
            label,
            progress,
        }
    }
}

impl<I: Iterator> Iterator for Counted<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<I::Item> {
        let item = self.inner.next()?;
        self.seen += 1;
        if self.seen.is_multiple_of(self.every) {
            self.progress
                .line(format_args!("{}: {} records read", self.label, self.seen));
        }
        Some(item)
    }
}

/// Reads an iterator on a thread of its own, a bounded number of items ahead of the consumer.
///
/// The importer pulls records on the task that writes to the database; reading and parsing
/// them here lets both run at once.
pub struct ReadAhead<T> {
    items: mpsc::Receiver<T>,
}

impl<T: Send + 'static> ReadAhead<T> {
    /// Starts reading `source`, staying at most `depth` items ahead.
    pub fn spawn(
        source: impl Iterator<Item = T> + Send + 'static,
        depth: usize,
    ) -> Result<Self, CliError> {
        let (sender, items) = mpsc::sync_channel(depth.max(1));
        std::thread::Builder::new()
            .name("mtg-cli-reader".into())
            .spawn(move || {
                for item in source {
                    if sender.send(item).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| CliError::io("reader thread", e))?;
        Ok(Self { items })
    }
}

impl<T> Iterator for ReadAhead<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.items.recv().ok()
    }
}

#[cfg(test)]
mod tests {
    //! Goal: cached files read the same whether gzipped or plain, framed as an array or as lines, with or without a BOM or leading blanks, and read-ahead keeps every item in order and ends when its consumer does.
    use std::fs;
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use rstest::rstest;

    use super::*;

    fn card(n: usize) -> String {
        format!(r#"{{"id":"p{n}","oracle_id":"o{n}","name":"Card {n}","cmc":1.0}}"#)
    }

    fn names(path: &Path) -> Vec<String> {
        open_cards(path)
            .unwrap()
            .map(|card| card.unwrap().name)
            .collect()
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn gzip(text: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(text).unwrap();
        encoder.finish().unwrap()
    }

    #[rstest]
    #[case::array(format!("[{},{}]", card(1), card(2)))]
    #[case::pretty_array(format!("\n  [\n{},\n{}\n]\n", card(1), card(2)))]
    #[case::lines(format!("{}\n{}\n", card(1), card(2)))]
    #[case::lines_with_blank_lines(format!("\n{}\r\n\r\n{}", card(1), card(2)))]
    #[case::array_with_bom(format!("\u{feff}[{},{}]", card(1), card(2)))]
    #[case::lines_with_bom(format!("\u{feff}{}\n{}\n", card(1), card(2)))]
    fn every_framing_yields_the_same_cards(#[case] text: String) {
        let dir = tempfile::tempdir().unwrap();
        let plain = write(dir.path(), "plain.json", text.as_bytes());
        let packed = write(dir.path(), "packed.json.gz", &gzip(text.as_bytes()));
        let expected = vec!["Card 1".to_owned(), "Card 2".to_owned()];
        assert_eq!(names(&plain), expected);
        assert_eq!(names(&packed), expected);
    }

    #[test]
    fn an_empty_file_has_no_records_and_a_missing_one_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            names(&write(dir.path(), "empty", b"")),
            Vec::<String>::new()
        );
        assert_eq!(
            names(&write(dir.path(), "blank.gz", &gzip(b"\n\n"))),
            Vec::<String>::new()
        );
        assert!(matches!(
            open_cards(&dir.path().join("absent")),
            Err(CliError::Io { .. })
        ));
    }

    #[test]
    fn concatenated_gzip_members_read_as_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = gzip(format!("{}\n", card(1)).as_bytes());
        bytes.extend(gzip(format!("{}\n", card(2)).as_bytes()));
        assert_eq!(
            names(&write(dir.path(), "two.gz", &bytes)),
            ["Card 1", "Card 2"]
        );
    }

    #[test]
    fn rulings_read_like_cards() {
        let dir = tempfile::tempdir().unwrap();
        let line =
            r#"{"oracle_id":"o","source":"wotc","published_at":"2020-01-01","comment":"Yes."}"#;
        let path = write(
            dir.path(),
            "r.jsonl.gz",
            &gzip(format!("{line}\n{line}\n").as_bytes()),
        );
        assert_eq!(open_rulings(&path).unwrap().count(), 2);
    }

    #[test]
    fn counting_reports_every_so_many_records_and_passes_all_of_them() {
        let (progress, captured) = Progress::buffer();
        let counted = Counted::new(0..7, "cards", 3, progress);
        assert_eq!(counted.collect::<Vec<_>>(), (0..7).collect::<Vec<_>>());
        let text = captured.text();
        assert_eq!(text, "cards: 3 records read\ncards: 6 records read\n");
    }

    #[rstest]
    #[case(1)]
    #[case(4)]
    #[case(1000)]
    fn read_ahead_keeps_every_item_in_order(#[case] depth: usize) {
        let ahead = ReadAhead::spawn(0..500, depth).unwrap();
        assert_eq!(ahead.collect::<Vec<_>>(), (0..500).collect::<Vec<_>>());
    }

    #[test]
    fn dropping_the_consumer_releases_the_reader() {
        let (done, finished) = mpsc::channel();
        let source = (0..).inspect(|_| {}).take_while(move |_| true);
        let mut ahead = ReadAhead::spawn(
            Dropping {
                inner: source,
                done,
            },
            2,
        )
        .unwrap();
        assert_eq!(ahead.next(), Some(0));
        drop(ahead);
        finished
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
    }

    /// An endless iterator that says so when it is dropped.
    struct Dropping<I> {
        inner: I,
        done: mpsc::Sender<()>,
    }

    impl<I: Iterator> Iterator for Dropping<I> {
        type Item = I::Item;

        fn next(&mut self) -> Option<I::Item> {
            self.inner.next()
        }
    }

    impl<I> Drop for Dropping<I> {
        fn drop(&mut self) {
            let _ = self.done.send(());
        }
    }
}
