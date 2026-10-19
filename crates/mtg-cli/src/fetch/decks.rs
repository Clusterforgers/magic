//! `fetch decks`: the MTGJSON deck index and the deck files chosen from it.

use std::cmp::Reverse;

use mtg_import::mtgjson::{DeckListEntry, parse_deck_list};
use reqwest::Url;
use serde::Deserialize;
use serde::de::IgnoredAny;

use super::{Endpoints, is_plain_file_name};
use crate::cache::{Cache, DeckRecord, FileRecord, IndexRecord, now};
use crate::error::{CliError, FetchError};
use crate::files::{human_size, sha256_hex, write_atomic};
use crate::http::{Http, Reply};
use crate::progress::Progress;

/// The deck type fetched when no other is asked for: the Commander precons.
pub const DEFAULT_TYPE: &str = "Commander Deck";

/// Which decks of the index to fetch.
#[derive(Debug, Clone, Default)]
pub struct DeckFilter {
    /// Deck types to keep, compared without case; empty means [`DEFAULT_TYPE`] and `all` means every type.
    pub types: Vec<String>,
    /// Keep decks whose name contains one of these (without case) or whose file name equals one; empty keeps all.
    pub only: Vec<String>,
    /// Keep at most this many of the newest decks.
    pub limit: Option<usize>,
}

/// What happened to the chosen decks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeckFetchReport {
    /// Decks the filter chose.
    pub selected: usize,
    /// Downloaded because they were missing or had changed.
    pub downloaded: usize,
    /// Cached, and the server confirmed they had not changed.
    pub not_modified: usize,
    /// Cached from the index build that is current, so nothing was asked.
    pub current: usize,
    /// Decks the server does not have or sent unreadably, by file name.
    pub missing: Vec<String>,
}

impl DeckFilter {
    fn wants_type(&self, deck_type: &str) -> bool {
        if self.types.is_empty() {
            return deck_type.eq_ignore_ascii_case(DEFAULT_TYPE);
        }
        self.types.iter().any(|wanted| {
            wanted.eq_ignore_ascii_case("all") || wanted.eq_ignore_ascii_case(deck_type)
        })
    }

    fn wants_name(&self, entry: &DeckListEntry) -> bool {
        let name = entry.name.to_lowercase();
        self.only.is_empty()
            || self.only.iter().any(|term| {
                let term = term.to_lowercase();
                name.contains(&term) || entry.file_name.to_lowercase() == term
            })
    }
}

/// Chooses decks from the index: by type and name, newest first, at most `limit`.
pub fn select(entries: &[DeckListEntry], filter: &DeckFilter) -> Vec<DeckListEntry> {
    let mut chosen: Vec<DeckListEntry> = entries
        .iter()
        .filter(|entry| filter.wants_type(&entry.deck_type) && filter.wants_name(entry))
        .filter(|entry| is_plain_file_name(&entry.file_name))
        .cloned()
        .collect();
    chosen.sort_by(|a, b| {
        Reverse(&a.release_date)
            .cmp(&Reverse(&b.release_date))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    chosen.truncate(filter.limit.unwrap_or(usize::MAX));
    chosen
}

#[derive(Deserialize)]
struct IndexMeta {
    meta: Option<Build>,
}

#[derive(Deserialize)]
struct Build {
    version: Option<String>,
}

/// Downloads the deck index and the chosen decks that are missing or changed.
///
/// A deck is asked about only when the index build differs from the one that last confirmed it,
/// and then conditionally, so an unchanged file costs a short answer instead of a download.
pub async fn fetch_decks(
    http: &Http,
    cache: &Cache,
    endpoints: &Endpoints,
    filter: &DeckFilter,
    force: bool,
    progress: &Progress,
) -> Result<DeckFetchReport, CliError> {
    let (index, build) = fetch_index(http, cache, endpoints, force, progress).await?;
    let entries = parse_deck_list(index.as_slice()).map_err(|e| FetchError::Invalid {
        url: endpoints.mtgjson_index.to_string(),
        reason: format!("not a deck index: {e}"),
    })?;
    let chosen = select(&entries, filter);
    progress.line(format_args!(
        "mtgjson: {} of {} decks selected",
        chosen.len(),
        entries.len()
    ));

    let mut report = DeckFetchReport {
        selected: chosen.len(),
        ..DeckFetchReport::default()
    };
    cache.prepare("mtgjson/decks")?;
    for (number, entry) in chosen.iter().enumerate() {
        let name = &entry.file_name;
        let mut manifest = cache.manifest()?;
        let known = manifest
            .mtgjson
            .decks
            .get(name)
            .filter(|r| cache.holds(&r.file));
        if let Some(record) = known
            && !force
            && build.is_some()
            && record.build == build
        {
            report.current += 1;
            continue;
        }
        let url = deck_url(&endpoints.mtgjson_decks, name)?;
        let validators = known.filter(|_| !force).map(|r| r.validators.clone());
        let reply = match http.get(&url, validators.as_ref()).await {
            Ok(reply) => reply,
            Err(error @ (FetchError::NotFound { .. } | FetchError::Invalid { .. })) => {
                progress.line(format_args!("mtgjson: skipping {name}: {error}"));
                report.missing.push(name.clone());
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        match reply {
            Reply::NotModified => {
                if let Some(record) = manifest.mtgjson.decks.get_mut(name) {
                    record.build.clone_from(&build);
                }
                report.not_modified += 1;
            }
            Reply::Modified { body, validators } => {
                if serde_json::from_slice::<IgnoredAny>(&body).is_err() {
                    progress.line(format_args!(
                        "mtgjson: skipping {name}: the file is not JSON"
                    ));
                    report.missing.push(name.clone());
                    continue;
                }
                let relative = format!("mtgjson/decks/{name}.json");
                let path = cache.path(&relative);
                write_atomic(&path, &body).map_err(|e| CliError::io(&path, e))?;
                manifest.mtgjson.decks.insert(
                    name.clone(),
                    DeckRecord {
                        file: FileRecord {
                            file: relative,
                            url: url.to_string(),
                            size: u64::try_from(body.len()).unwrap_or(u64::MAX),
                            sha256: sha256_hex(&body),
                            fetched_at: now(),
                        },
                        build: build.clone(),
                        validators,
                    },
                );
                report.downloaded += 1;
            }
        }
        cache.save(&manifest)?;
        if (number + 1) % 25 == 0 {
            progress.line(format_args!(
                "mtgjson: {} of {} decks done",
                number + 1,
                chosen.len()
            ));
        }
    }
    progress.line(format_args!(
        "mtgjson: {} downloaded, {} unchanged, {} current, {} missing",
        report.downloaded,
        report.not_modified,
        report.current,
        report.missing.len()
    ));
    Ok(report)
}

/// Fetches the index (conditionally) and returns its bytes and the MTGJSON build it describes.
async fn fetch_index(
    http: &Http,
    cache: &Cache,
    endpoints: &Endpoints,
    force: bool,
    progress: &Progress,
) -> Result<(Vec<u8>, Option<String>), CliError> {
    let mut manifest = cache.manifest()?;
    let cached = manifest
        .mtgjson
        .index
        .clone()
        .filter(|record| cache.holds(&record.file));
    let validators = cached
        .as_ref()
        .filter(|_| !force)
        .map(|record| record.validators.clone());
    let url = &endpoints.mtgjson_index;
    match http.get(url, validators.as_ref()).await? {
        Reply::NotModified => {
            let Some(record) = cached else {
                return Err(FetchError::Invalid {
                    url: url.to_string(),
                    reason: "304 although nothing is cached".into(),
                }
                .into());
            };
            let path = cache.path(&record.file.file);
            let bytes = std::fs::read(&path).map_err(|e| CliError::io(&path, e))?;
            progress.line(format_args!(
                "mtgjson: deck index unchanged (build {})",
                record.build.as_deref().unwrap_or("unknown")
            ));
            Ok((bytes, record.build))
        }
        Reply::Modified { body, validators } => {
            let build = serde_json::from_slice::<IndexMeta>(&body)
                .ok()
                .and_then(|index| index.meta)
                .and_then(|meta| meta.version);
            let relative = "mtgjson/DeckList.json".to_owned();
            cache.prepare("mtgjson")?;
            let path = cache.path(&relative);
            write_atomic(&path, &body).map_err(|e| CliError::io(&path, e))?;
            manifest.mtgjson.index = Some(IndexRecord {
                file: FileRecord {
                    file: relative,
                    url: url.to_string(),
                    size: u64::try_from(body.len()).unwrap_or(u64::MAX),
                    sha256: sha256_hex(&body),
                    fetched_at: now(),
                },
                build: build.clone(),
                validators,
            });
            cache.save(&manifest)?;
            progress.line(format_args!(
                "mtgjson: deck index saved, {} (build {})",
                human_size(u64::try_from(body.len()).unwrap_or(u64::MAX)),
                build.as_deref().unwrap_or("unknown")
            ));
            Ok((body, build))
        }
    }
}

fn deck_url(base: &Url, file_name: &str) -> Result<Url, FetchError> {
    let mut url = base.clone();
    url.path_segments_mut()
        .map_err(|()| FetchError::Invalid {
            url: base.to_string(),
            reason: "cannot take path segments".into(),
        })?
        .pop_if_empty()
        .push(&format!("{file_name}.json"));
    Ok(url)
}

#[cfg(test)]
mod tests {
    //! Goal: the deck filter picks by type and name without regard to case, orders newest first, honours the limit and never lets an unsafe file name through; deck addresses encode any name.
    use rstest::rstest;

    use super::*;

    fn entry(file: &str, name: &str, kind: &str, date: Option<&str>) -> DeckListEntry {
        DeckListEntry {
            code: "TST".into(),
            file_name: file.into(),
            name: name.into(),
            release_date: date.map(str::to_owned),
            deck_type: kind.into(),
        }
    }

    fn index() -> Vec<DeckListEntry> {
        vec![
            entry("Old_TST", "Old Guard", "Commander Deck", Some("2012-06-15")),
            entry("New_TST", "New Guard", "Commander Deck", Some("2026-10-02")),
            entry(
                "Mid_TST",
                "Middle Guard",
                "Commander Deck",
                Some("2020-01-01"),
            ),
            entry("Undated_TST", "Undated Guard", "Commander Deck", None),
            entry("Theme_TST", "Guard Theme", "Theme Deck", Some("2007-07-13")),
            entry(
                "../evil",
                "Guard Evil",
                "Commander Deck",
                Some("2030-01-01"),
            ),
        ]
    }

    fn files(chosen: &[DeckListEntry]) -> Vec<&str> {
        chosen.iter().map(|e| e.file_name.as_str()).collect()
    }

    #[rstest]
    #[case::default_is_commander_newest_first(&[], &[], None, &["New_TST", "Mid_TST", "Old_TST", "Undated_TST"])]
    #[case::limit_keeps_the_newest(&[], &[], Some(2), &["New_TST", "Mid_TST"])]
    #[case::limit_zero_keeps_nothing(&[], &[], Some(0), &[])]
    #[case::other_type_without_case(&["theme deck"], &[], None, &["Theme_TST"])]
    #[case::all_types(&["all"], &[], None, &["New_TST", "Mid_TST", "Old_TST", "Theme_TST", "Undated_TST"])]
    #[case::name_part_without_case(&[], &["MIDDLE"], None, &["Mid_TST"])]
    #[case::file_name_exactly(&[], &["old_tst"], None, &["Old_TST"])]
    #[case::any_of_several_terms(&[], &["old", "new"], None, &["New_TST", "Old_TST"])]
    #[case::no_match(&[], &["nothing like it"], None, &[])]
    #[case::terms_and_limit(&[], &["guard"], Some(1), &["New_TST"])]
    fn decks_are_chosen_by_type_name_and_age(
        #[case] types: &[&str],
        #[case] only: &[&str],
        #[case] limit: Option<usize>,
        #[case] expected: &[&str],
    ) {
        let filter = DeckFilter {
            types: types.iter().map(|s| (*s).to_owned()).collect(),
            only: only.iter().map(|s| (*s).to_owned()).collect(),
            limit,
        };
        let chosen = select(&index(), &filter);
        assert_eq!(files(&chosen), expected);
    }

    #[test]
    fn an_unsafe_file_name_is_never_chosen_even_when_asked_for() {
        let filter = DeckFilter {
            only: vec!["evil".into()],
            ..DeckFilter::default()
        };
        assert_eq!(files(&select(&index(), &filter)), Vec::<&str>::new());
    }

    #[rstest]
    #[case("Deck_ABC", "https://h.test/api/v5/decks/Deck_ABC.json")]
    #[case("Mah\u{14d}Gaku", "https://h.test/api/v5/decks/Mah%C5%8DGaku.json")]
    #[case("A B", "https://h.test/api/v5/decks/A%20B.json")]
    #[case("what?is#this", "https://h.test/api/v5/decks/what%3Fis%23this.json")]
    fn deck_addresses_encode_any_name(#[case] name: &str, #[case] expected: &str) {
        let base = Url::parse("https://h.test/api/v5/decks/").unwrap();
        assert_eq!(deck_url(&base, name).unwrap().as_str(), expected);
    }
}
