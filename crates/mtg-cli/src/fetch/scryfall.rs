//! `fetch scryfall`: reads the bulk-data listing and downloads the bulk files the importer reads.

use std::path::{Path, PathBuf};

use reqwest::Url;
use serde::Deserialize;

use super::is_plain_file_name;
use crate::cache::{BulkRecord, Cache, FileRecord, SCRYFALL_TYPES, now};
use crate::error::{CliError, FetchError};
use crate::files::{human_size, is_gzip, sha256_file, short, verify_gzip};
use crate::http::{Http, Reply};
use crate::progress::Progress;

/// What a bulk file needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkAction {
    /// The cached file came from the listing's current `updated_at`.
    UpToDate,
    /// The file was downloaded; `resumed_from` bytes of it were already there.
    Downloaded {
        /// Bytes of a partial file that were kept.
        resumed_from: u64,
    },
}

/// The outcome for one bulk type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOutcome {
    /// The bulk type, such as `oracle_cards`.
    pub kind: &'static str,
    /// What was done.
    pub action: BulkAction,
    /// The cache's record of the file now.
    pub record: BulkRecord,
}

#[derive(Deserialize)]
struct Listing {
    data: Vec<ListingEntry>,
}

#[derive(Deserialize)]
struct ListingEntry {
    #[serde(rename = "type")]
    kind: String,
    updated_at: String,
    #[serde(default)]
    jsonl_download_uri: Option<String>,
    #[serde(default)]
    download_uri: Option<String>,
    #[serde(default)]
    compressed_size: Option<u64>,
}

/// Downloads the Scryfall bulk files that are missing or older than the listing says.
///
/// The files are the Oracle Cards (one printing per card) and the rulings. A file is left alone
/// when the manifest holds it under the listing's `updated_at`, unless `force` is set.
pub async fn fetch_scryfall(
    http: &Http,
    cache: &Cache,
    listing_url: &Url,
    force: bool,
    progress: &Progress,
) -> Result<Vec<BulkOutcome>, CliError> {
    let listing = read_listing(http, listing_url).await?;
    // Every entry is checked before the first download starts.
    let mut wanted = Vec::new();
    for kind in SCRYFALL_TYPES {
        let entry = listing
            .data
            .iter()
            .find(|entry| entry.kind == kind)
            .ok_or_else(|| invalid(listing_url, format!("the listing has no `{kind}` entry")))?;
        wanted.push(resolve(kind, entry, listing_url)?);
    }
    let mut outcomes = Vec::new();
    for bulk in &wanted {
        outcomes.push(fetch_bulk(http, cache, bulk, force, progress).await?);
    }
    Ok(outcomes)
}

async fn read_listing(http: &Http, url: &Url) -> Result<Listing, FetchError> {
    let Reply::Modified { body, .. } = http.get(url, None).await? else {
        return Err(invalid(
            url,
            "the server answered 304 to an unconditional request",
        ));
    };
    serde_json::from_slice(&body).map_err(|e| invalid(url, format!("not a bulk-data listing: {e}")))
}

fn invalid(url: &Url, reason: impl Into<String>) -> FetchError {
    FetchError::Invalid {
        url: url.to_string(),
        reason: reason.into(),
    }
}

/// A listing entry with its download address checked.
struct Wanted<'a> {
    kind: &'static str,
    entry: &'a ListingEntry,
    url: Url,
    name: String,
}

/// Finds the address of a listing entry and the cache file name it implies.
fn resolve<'a>(
    kind: &'static str,
    entry: &'a ListingEntry,
    listing_url: &Url,
) -> Result<Wanted<'a>, FetchError> {
    let source = entry
        .jsonl_download_uri
        .as_deref()
        .or(entry.download_uri.as_deref())
        .ok_or_else(|| invalid(listing_url, format!("`{kind}` has no download address")))?;
    let url = Url::parse(source)
        .map_err(|e| invalid(listing_url, format!("`{kind}` address {source:?}: {e}")))?;
    let name = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|name| is_plain_file_name(name))
        .map(str::to_owned)
        .ok_or_else(|| {
            invalid(
                listing_url,
                format!("`{kind}` address {source:?} names no file"),
            )
        })?;
    Ok(Wanted {
        kind,
        entry,
        url,
        name,
    })
}

async fn fetch_bulk(
    http: &Http,
    cache: &Cache,
    wanted: &Wanted<'_>,
    force: bool,
    progress: &Progress,
) -> Result<BulkOutcome, CliError> {
    let (kind, entry) = (wanted.kind, wanted.entry);
    let (url, name) = (&wanted.url, wanted.name.as_str());
    let relative = format!("scryfall/{name}");

    let mut manifest = cache.manifest()?;
    if let Some(record) = manifest.scryfall.get(kind)
        && !force
        && record.updated_at == entry.updated_at
        && record.file.file == relative
        && cache.holds(&record.file)
    {
        progress.line(format_args!(
            "scryfall: {kind} is up to date (updated {})",
            entry.updated_at
        ));
        return Ok(BulkOutcome {
            kind,
            action: BulkAction::UpToDate,
            record: record.clone(),
        });
    }

    let target = cache.path(&relative);
    let part = partial_path(&target);
    cache.prepare("scryfall")?;
    progress.line(format_args!(
        "scryfall: downloading {kind} {}from {url}",
        entry
            .compressed_size
            .map(|size| format!("({}) ", human_size(size)))
            .unwrap_or_default()
    ));
    let packed = Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gz"));
    let expected = entry.compressed_size.filter(|_| packed);
    let downloaded = http.download(url, &part, expected).await?;

    let (sha256, size) = verified_checksum(&part, url).await?;
    tokio::fs::rename(&part, &target)
        .await
        .map_err(|e| CliError::io(&target, e))?;

    let record = BulkRecord {
        file: FileRecord {
            file: relative,
            url: url.to_string(),
            size,
            sha256,
            fetched_at: now(),
        },
        updated_at: entry.updated_at.clone(),
    };
    let superseded = manifest.scryfall.insert(kind.to_owned(), record.clone());
    cache.save(&manifest)?;
    if let Some(old) = superseded
        && old.file.file != record.file.file
    {
        let _ = std::fs::remove_file(cache.path(&old.file.file));
    }
    progress.line(format_args!(
        "scryfall: {kind} saved, {}{}, sha256 {}",
        human_size(size),
        if downloaded.resumed_from > 0 {
            format!(" (resumed after {})", human_size(downloaded.resumed_from))
        } else {
            String::new()
        },
        short(&record.file.sha256)
    ));
    Ok(BulkOutcome {
        kind,
        action: BulkAction::Downloaded {
            resumed_from: downloaded.resumed_from,
        },
        record,
    })
}

/// The partial file next to a target.
pub fn partial_path(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_default();
    name.push(".part");
    target.with_file_name(name)
}

/// Checks a finished download and returns its checksum and length; a broken gzip file is deleted.
async fn verified_checksum(part: &Path, url: &Url) -> Result<(String, u64), CliError> {
    /// Why a downloaded file failed its checks.
    enum Broken {
        Gzip(std::io::Error),
        Io(std::io::Error),
    }
    let path = part.to_path_buf();
    let checked = tokio::task::spawn_blocking(move || {
        if is_gzip(&path).map_err(Broken::Io)? {
            verify_gzip(&path).map_err(Broken::Gzip)?;
        }
        sha256_file(&path).map_err(Broken::Io)
    })
    .await
    .map_err(|e| CliError::Config(format!("the checksum task failed: {e}")))?;
    match checked {
        Ok(checksum) => Ok(checksum),
        Err(Broken::Gzip(error)) => {
            let _ = tokio::fs::remove_file(part).await;
            Err(FetchError::Corrupt {
                url: url.to_string(),
                reason: format!(
                    "the downloaded file is not a whole gzip file ({error}); it was deleted"
                ),
            }
            .into())
        }
        Err(Broken::Io(error)) => Err(CliError::io(part, error)),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the partial file of a download sits next to its target under a name that can never be mistaken for a finished file.
    use super::*;

    #[test]
    fn the_partial_file_is_a_sibling_with_a_part_suffix() {
        let target = Path::new("/cache/scryfall/oracle-cards-1.jsonl.gz");
        assert_eq!(
            partial_path(target),
            Path::new("/cache/scryfall/oracle-cards-1.jsonl.gz.part")
        );
    }
}
