//! The cost ledger of the paid API: one tab-separated line for each billed request, appended as it is billed, so that the
//! spend survives a restart and counts toward the cap of every later run.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};

use super::model::Usage;
use super::price::Usd;
use crate::cache::Cache;
use crate::error::CliError;

/// The file in the cache directory.
pub const LEDGER_FILE: &str = "api-ledger.tsv";

/// The first line of the file.
const HEADER: &str =
    "time\tcard\tmodel\tinput_tokens\tcache_write_tokens\tcache_read_tokens\toutput_tokens\tusd";

/// The card column of the line that holds a batch's worst case while it is out.
pub fn reserved_label(batch: &str) -> String {
    format!("[batch {batch} reserved]")
}

/// The card column of the line that returns a batch's reservation once its results are billed.
pub fn settled_label(batch: &str) -> String {
    format!("[batch {batch} settled]")
}

/// One line of the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The card the request was about, or a bracketed label for a line that is no request.
    pub card: String,
    /// The model asked.
    pub model: String,
    /// The tokens billed.
    pub usage: Usage,
    /// The cost in micro-dollars; negative for the line that returns a reservation.
    pub micros: i64,
}

/// The ledger file.
#[derive(Debug, Clone)]
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    /// The ledger of the cache directory.
    pub fn of(cache: &Cache) -> Self {
        Self::at(cache.path(LEDGER_FILE))
    }

    /// A ledger in a file of its own.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The text of the file; none is an empty ledger.
    fn read(&self) -> Result<String, CliError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(error) => Err(CliError::io(&self.path, error)),
        }
    }

    /// The lines of the file as entries. A line that cannot be read is an error, never a zero: a ledger that undercounts
    /// would let a run pass its cap.
    fn lines(&self) -> Result<Vec<Entry>, CliError> {
        let text = self.read()?;
        let mut entries = Vec::new();
        for (at, line) in text.lines().enumerate() {
            if line.trim().is_empty() || line == HEADER {
                continue;
            }
            let entry = parse_line(line).ok_or_else(|| CliError::Cache {
                path: self.path.clone(),
                reason: format!(
                    "line {} is not a ledger line; fix or remove it, the cap counts this file",
                    at + 1
                ),
            })?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// What the ledger holds in all: the cost of every billed request and the worst cases of batches still out.
    pub fn total(&self) -> Result<i64, CliError> {
        Ok(self.lines()?.iter().map(|entry| entry.micros).sum())
    }

    /// Whether a line with this card column is in the ledger.
    pub fn has_label(&self, label: &str) -> Result<bool, CliError> {
        Ok(self.lines()?.iter().any(|entry| entry.card == label))
    }

    /// Appends the entries in one write, the header first when the file is new.
    pub fn append(&self, entries: &[Entry]) -> Result<(), CliError> {
        if entries.is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| CliError::io(parent, error))?;
        }
        let mut text = String::new();
        let new = fs::metadata(&self.path).map_or(true, |meta| meta.len() == 0);
        if new {
            text.push_str(HEADER);
            text.push('\n');
        }
        let time = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        for entry in entries {
            text.push_str(&format_line(&time, entry));
            text.push('\n');
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| CliError::io(&self.path, error))?;
        file.write_all(text.as_bytes())
            .and_then(|()| file.sync_data())
            .map_err(|error| CliError::io(&self.path, error))
    }
}

/// A card name as one column: no tab or line break.
fn column(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

fn signed(micros: i64) -> String {
    let sign = if micros < 0 { "-" } else { "" };
    format!("{sign}{}", Usd(micros.unsigned_abs()).plain())
}

fn format_line(time: &str, entry: &Entry) -> String {
    format!(
        "{time}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        column(&entry.card),
        column(&entry.model),
        entry.usage.uncached_tokens(),
        entry.usage.cache_write_tokens,
        entry.usage.cache_read_tokens,
        entry.usage.output_tokens,
        signed(entry.micros)
    )
}

fn parse_micros(text: &str) -> Option<i64> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = digits.split_once('.')?;
    if fraction.len() != 6 || whole.is_empty() {
        return None;
    }
    let whole: i64 = whole.parse().ok()?;
    let fraction: i64 = fraction.parse().ok()?;
    let micros = whole.checked_mul(1_000_000)?.checked_add(fraction)?;
    Some(if negative { -micros } else { micros })
}

fn parse_line(line: &str) -> Option<Entry> {
    let columns: Vec<&str> = line.split('\t').collect();
    let [_time, card, model, uncached, write, read, output, usd] = columns.as_slice() else {
        return None;
    };
    let (uncached, write, read): (u64, u64, u64) = (
        uncached.parse().ok()?,
        write.parse().ok()?,
        read.parse().ok()?,
    );
    Some(Entry {
        card: (*card).to_owned(),
        model: (*model).to_owned(),
        usage: Usage {
            input_tokens: uncached + write + read,
            cache_write_tokens: write,
            cache_read_tokens: read,
            output_tokens: output.parse().ok()?,
        },
        micros: parse_micros(usd)?,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the ledger is a TSV with a header that every billed request appends to, its total survives a reopen and
    //! counts reservations and their return, names with tabs stay one column, and a line that cannot be read is an error
    //! rather than a zero.
    use rstest::rstest;

    use super::*;

    fn entry(card: &str, micros: i64) -> Entry {
        Entry {
            card: card.into(),
            model: "claude-sonnet-5-5".into(),
            usage: Usage {
                input_tokens: 1_200,
                cache_write_tokens: 200,
                cache_read_tokens: 900,
                output_tokens: 80,
            },
            micros,
        }
    }

    #[test]
    fn lines_are_appended_under_one_header_and_sum_after_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/api-ledger.tsv");
        let ledger = Ledger::at(&path);
        assert_eq!(ledger.total().unwrap(), 0);
        ledger.append(&[entry("Opt", 12_345)]).unwrap();
        ledger
            .append(&[entry("Wind Drake", 1_000_000), entry("Serra Angel", 5)])
            .unwrap();
        let reopened = Ledger::at(&path);
        assert_eq!(reopened.total().unwrap(), 1_012_350);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("time\tcard").count(), 1);
        let line = text.lines().nth(1).unwrap();
        let columns: Vec<&str> = line.split('\t').collect();
        assert_eq!(
            &columns[1..],
            [
                "Opt",
                "claude-sonnet-5-5",
                "100",
                "200",
                "900",
                "80",
                "0.012345"
            ]
        );
        assert!(
            columns[0].ends_with('Z') && columns[0].contains('T'),
            "{line}"
        );
    }

    #[test]
    fn a_returned_reservation_is_a_negative_line_and_labels_are_found() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(dir.path().join("l.tsv"));
        let batch = "msgbatch_01";
        ledger
            .append(&[Entry {
                card: reserved_label(batch),
                model: "m".into(),
                usage: Usage::default(),
                micros: 3_000_000,
            }])
            .unwrap();
        assert_eq!(ledger.total().unwrap(), 3_000_000);
        assert!(!ledger.has_label(&settled_label(batch)).unwrap());
        ledger
            .append(&[
                entry("Opt", 40_000),
                Entry {
                    card: settled_label(batch),
                    model: "m".into(),
                    usage: Usage::default(),
                    micros: -3_000_000,
                },
            ])
            .unwrap();
        assert_eq!(ledger.total().unwrap(), 40_000);
        assert!(ledger.has_label(&settled_label(batch)).unwrap());
        let text = std::fs::read_to_string(ledger.path()).unwrap();
        assert!(text.contains("\t-3.000000"), "{text}");
    }

    #[test]
    fn a_name_with_a_tab_or_a_line_break_stays_one_column() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Ledger::at(dir.path().join("l.tsv"));
        ledger.append(&[entry("odd\tname\nhere", 7)]).unwrap();
        assert_eq!(ledger.total().unwrap(), 7);
        assert!(ledger.has_label("odd name here").unwrap());
    }

    #[rstest]
    #[case::too_few_columns("2026-11-10T10:00:00Z\tOpt\tm\t1\t2")]
    #[case::words_for_tokens("2026-11-10T10:00:00Z\tOpt\tm\tmany\t0\t0\t0\t0.000001")]
    #[case::no_decimals("2026-11-10T10:00:00Z\tOpt\tm\t1\t0\t0\t1\t5")]
    #[case::a_comma("2026-11-10T10:00:00Z\tOpt\tm\t1\t0\t0\t1\t0,5")]
    fn a_line_that_cannot_be_read_is_an_error_not_a_zero(#[case] line: &str) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("l.tsv");
        std::fs::write(&path, format!("{HEADER}\n{line}\n")).unwrap();
        let error = Ledger::at(&path).total().unwrap_err().to_string();
        assert!(error.contains("line 2"), "{error}");
    }
}
