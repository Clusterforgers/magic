//! Dry-run checks: the record rules of the importers applied to a stream, with no database.

use mtg_import::mtgjson::{MtgjsonError, PreconDeck};
use mtg_import::scryfall::{CardRecord, RulingRecord, ScryfallError};
use serde::Serialize;

use super::batches::SkipLog;
use super::card_payload::split_record;
use super::precons::validate_deck;
use super::rulings::ruling_row;

/// What an import would do with a record stream: how many records it stores and which it skips.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CheckReport {
    /// Records the importer would store.
    pub usable: u64,
    /// Records it would skip, with the stream positions and reasons it would report.
    pub skipped: SkipLog,
}

/// Applies the card import's record rules to `records`; a fatal stream error ends the check.
pub fn check_cards<I>(records: I) -> Result<CheckReport, ScryfallError>
where
    I: IntoIterator<Item = Result<CardRecord, ScryfallError>>,
{
    check(records, |record| split_record(record).map(|_| ()))
}

/// Applies the ruling import's record rules to `records`; a fatal stream error ends the check.
pub fn check_rulings<I>(records: I) -> Result<CheckReport, ScryfallError>
where
    I: IntoIterator<Item = Result<RulingRecord, ScryfallError>>,
{
    check(records, |record| ruling_row(0, record).map(|_| ()))
}

/// Applies the precon import's deck rules to `decks`.
pub fn check_precons<I>(decks: I) -> CheckReport
where
    I: IntoIterator<Item = Result<PreconDeck, MtgjsonError>>,
{
    let mut report = CheckReport::default();
    for (position, deck) in (0u64..).zip(decks) {
        match deck {
            Ok(deck) => match validate_deck(&deck) {
                Ok(_) => report.usable += 1,
                Err(rejection) => report
                    .skipped
                    .record(position, format!("{}: {rejection}", deck.name)),
            },
            Err(error) => report.skipped.record(position, error),
        }
    }
    report
}

fn check<T, R: std::fmt::Display>(
    records: impl IntoIterator<Item = Result<T, ScryfallError>>,
    rule: impl Fn(&T) -> Result<(), R>,
) -> Result<CheckReport, ScryfallError> {
    let mut report = CheckReport::default();
    for (position, record) in (0u64..).zip(records) {
        match record {
            Ok(record) => match rule(&record) {
                Ok(()) => report.usable += 1,
                Err(rejection) => report.skipped.record(position, rejection),
            },
            Err(error) if error.is_fatal() => return Err(error),
            Err(error) => report.skipped.record(position, error),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    //! Goal: a dry run counts exactly the records the importers would store and skip, reports a fatal stream error, and never needs a database.
    use mtg_import::mtgjson::parse_deck_str;
    use mtg_import::scryfall::{cards, rulings};
    use rstest::rstest;

    use super::*;

    fn card(n: usize, extra: &str) -> String {
        format!(
            r#"{{"id":"{:08x}-0000-0000-0000-000000000000","oracle_id":"{:08x}-0000-0000-0000-000000000001","name":"Card {n}"{extra}}}"#,
            n + 1,
            n + 1
        )
    }

    #[rstest]
    #[case::all_usable(&[card(0, ""), card(1, "")], 2, &[])]
    #[case::unparsable_entry(&[card(0, ""), r#"{"id":"x"}"#.to_owned()], 1, &[1])]
    #[case::bad_date(&[card(0, r#","released_at":"2009-02-30""#), card(1, "")], 1, &[0])]
    #[case::nul_byte(&[card(0, ""), card(1, r#","oracle_text":"a\u0000b""#)], 1, &[1])]
    #[case::bad_ids(&[r#"{"id":"p","oracle_id":"o","name":"N"}"#.to_owned()], 0, &[0])]
    fn cards_are_counted_like_the_import_counts_them(
        #[case] entries: &[String],
        #[case] usable: u64,
        #[case] skipped_at: &[u64],
    ) {
        let json = format!("[{}]", entries.join(","));
        let report = check_cards(cards(json.as_bytes())).unwrap();
        assert_eq!(report.usable, usable);
        assert_eq!(report.skipped.count, skipped_at.len() as u64);
        let positions: Vec<u64> = report.skipped.samples.iter().map(|s| s.position).collect();
        assert_eq!(positions, skipped_at);
    }

    #[test]
    fn a_fatal_stream_error_ends_the_check() {
        let json = format!("[{},{{\"id\":", card(0, ""));
        assert!(check_cards(cards(json.as_bytes())).unwrap_err().is_fatal());
    }

    #[test]
    fn nul_bytes_are_named_and_never_echoed() {
        let json = format!(
            "[{}]",
            card(0, r#","name":"Bolt\u0000""#).replace(r#""name":"Card 0","#, "")
        );
        let report = check_cards(cards(json.as_bytes())).unwrap();
        assert_eq!(report.skipped.count, 1);
        assert!(report.skipped.samples[0].reason.contains("NUL"));
        assert!(!report.skipped.samples[0].reason.contains('\0'));
    }

    #[test]
    fn rulings_with_unusable_fields_are_skipped() {
        let oracle = "00000001-0000-0000-0000-000000000001";
        let json = format!(
            r#"[{{"oracle_id":"{oracle}","source":"wotc","published_at":"2020-01-01","comment":"Fine."}},
                {{"oracle_id":"{oracle}","source":"wotc","published_at":"2020-01-01","comment":"Bad\u0000"}},
                {{"oracle_id":"{oracle}","source":"wotc","published_at":"2020-13-01","comment":"Date."}},
                {{"oracle_id":"nope","source":"wotc","published_at":"2020-01-01","comment":"Id."}},
                {{"oracle_id":"{oracle}","source":"wotc","published_at":"2020-01-01","comment":"  "}}]"#
        );
        let report = check_rulings(rulings(json.as_bytes())).unwrap();
        assert_eq!(report.usable, 1);
        let positions: Vec<u64> = report.skipped.samples.iter().map(|s| s.position).collect();
        assert_eq!(positions, [1, 2, 3, 4]);
    }

    #[rstest]
    #[case::fine(r#"{"name":"D","code":"C","releaseDate":"2020-01-01"}"#, 1, 0)]
    #[case::nul_in_the_name("{\"name\":\"D\\u0000\",\"code\":\"C\"}", 0, 1)]
    #[case::nul_in_a_card(
        "{\"name\":\"D\",\"code\":\"C\",\"mainBoard\":[{\"count\":1,\"uuid\":\"u\",\"name\":\"A\\u0000\"}]}",
        0,
        1
    )]
    #[case::bad_date(r#"{"name":"D","code":"C","releaseDate":"soon"}"#, 0, 1)]
    #[case::not_a_deck(r#"{"name":"D"}"#, 0, 1)]
    fn decks_are_counted_like_the_import_counts_them(
        #[case] json: &str,
        #[case] usable: u64,
        #[case] skipped: u64,
    ) {
        let report = check_precons([parse_deck_str(json)]);
        assert_eq!((report.usable, report.skipped.count), (usable, skipped));
    }
}
