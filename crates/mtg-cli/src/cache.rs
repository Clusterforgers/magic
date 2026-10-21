//! The download cache: where files live and the manifest that says which of them are complete and current.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::error::CliError;
use crate::files::write_atomic;
use crate::http::Validators;

/// Name of the manifest inside the cache directory.
const MANIFEST: &str = "manifest.json";

/// Scryfall bulk types every `fetch scryfall` downloads.
pub const SCRYFALL_TYPES: [&str; 2] = ["oracle_cards", "rulings"];

/// The bulk type with every printing of every card, downloaded on request (`--printings`).
pub const PRINTINGS_TYPE: &str = "default_cards";

/// A cache directory.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

/// What the cache knows about one complete file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    /// Path of the file below the cache directory, with `/` separators.
    pub file: String,
    /// Where it was downloaded from.
    pub url: String,
    /// Length in bytes.
    pub size: u64,
    /// SHA-256 of its bytes, in hexadecimal.
    pub sha256: String,
    /// When it was downloaded, as RFC 3339 UTC.
    pub fetched_at: String,
}

/// A Scryfall bulk file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkRecord {
    /// The file.
    #[serde(flatten)]
    pub file: FileRecord,
    /// The `updated_at` of the bulk-data listing entry it came from.
    pub updated_at: String,
}

/// The MTGJSON deck index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexRecord {
    /// The file.
    #[serde(flatten)]
    pub file: FileRecord,
    /// The MTGJSON build the index describes, such as `5.3.0+20261018`.
    pub build: Option<String>,
    /// What the server said about this copy.
    #[serde(flatten)]
    pub validators: Validators,
}

/// One MTGJSON deck file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckRecord {
    /// The file.
    #[serde(flatten)]
    pub file: FileRecord,
    /// The MTGJSON build whose index last confirmed this copy.
    pub build: Option<String>,
    /// What the server said about this copy.
    #[serde(flatten)]
    pub validators: Validators,
}

/// MTGJSON files by role.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MtgjsonRecords {
    /// The deck index.
    #[serde(default)]
    pub index: Option<IndexRecord>,
    /// Deck files by the index's `fileName`.
    #[serde(default)]
    pub decks: BTreeMap<String, DeckRecord>,
}

/// Which files in the cache are complete, and what they were downloaded as.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Scryfall bulk files by bulk type (`oracle_cards`, `rulings`, `default_cards`).
    #[serde(default)]
    pub scryfall: BTreeMap<String, BulkRecord>,
    /// MTGJSON files.
    #[serde(default)]
    pub mtgjson: MtgjsonRecords,
}

/// The current time as the manifest writes it.
pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// The directory used when neither a flag nor `MTG_CACHE_DIR` names one, from the user's cache location.
pub fn default_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let non_empty = |name: &str| var(name).filter(|value| !value.is_empty());
    if let Some(base) = non_empty("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Some(base.join("mtg-cli"));
    }
    let home = PathBuf::from(non_empty("HOME")?);
    let below_home = if cfg!(target_os = "macos") {
        "Library/Caches"
    } else {
        ".cache"
    };
    Some(home.join(below_home).join("mtg-cli"))
}

impl Cache {
    /// A cache in `root`, which is created when the first file is written.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The cache directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The absolute path of a manifest-relative file name.
    pub fn path(&self, relative: &str) -> PathBuf {
        relative
            .split('/')
            .fold(self.root.clone(), |path, part| path.join(part))
    }

    /// Creates the directory a file will be written into.
    pub fn prepare(&self, relative_dir: &str) -> Result<PathBuf, CliError> {
        let dir = self.path(relative_dir);
        fs::create_dir_all(&dir).map_err(|e| CliError::io(&dir, e))?;
        Ok(dir)
    }

    /// Reads the manifest; a cache without one is empty.
    pub fn manifest(&self) -> Result<Manifest, CliError> {
        let path = self.root.join(MANIFEST);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| CliError::Cache {
                path,
                reason: format!("the manifest is not valid: {e}"),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Manifest::default()),
            Err(error) => Err(CliError::io(path, error)),
        }
    }

    /// Replaces the manifest on disk in one step.
    pub fn save(&self, manifest: &Manifest) -> Result<(), CliError> {
        fs::create_dir_all(&self.root).map_err(|e| CliError::io(&self.root, e))?;
        let path = self.root.join(MANIFEST);
        let json = serde_json::to_vec_pretty(manifest).map_err(|e| CliError::Cache {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        write_atomic(&path, &json).map_err(|e| CliError::io(path, e))
    }

    /// Whether `record` still describes a file on disk of the recorded length.
    pub fn holds(&self, record: &FileRecord) -> bool {
        fs::metadata(self.path(&record.file)).is_ok_and(|m| m.is_file() && m.len() == record.size)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the default cache directory follows the user's environment, the manifest survives a round trip and a missing file, and a record only counts while its file is whole.
    use rstest::rstest;

    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[rstest]
    #[case::xdg_wins(&[("XDG_CACHE_HOME", "/x/cache"), ("HOME", "/home/u")], Some("/x/cache/mtg-cli"))]
    #[case::home_fallback(&[("HOME", "/home/u")], Some("/home/u/.cache/mtg-cli"))]
    #[case::relative_xdg_is_ignored(&[("XDG_CACHE_HOME", "cache"), ("HOME", "/home/u")], Some("/home/u/.cache/mtg-cli"))]
    #[case::empty_xdg_is_ignored(&[("XDG_CACHE_HOME", ""), ("HOME", "/home/u")], Some("/home/u/.cache/mtg-cli"))]
    #[case::nowhere_to_put_it(&[], None)]
    #[case::empty_home(&[("HOME", "")], None)]
    fn the_default_directory_follows_the_environment(
        #[case] vars: &[(&str, &str)],
        #[case] expected: Option<&str>,
    ) {
        if cfg!(target_os = "macos") {
            return;
        }
        assert_eq!(default_dir(env(vars)), expected.map(PathBuf::from));
    }

    fn record(file: &str, size: u64) -> FileRecord {
        FileRecord {
            file: file.into(),
            url: "http://example.test/f".into(),
            size,
            sha256: "ab".repeat(32),
            fetched_at: "2026-10-19T08:00:00Z".into(),
        }
    }

    #[test]
    fn the_manifest_round_trips_and_a_missing_one_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join("deep/er"));
        assert_eq!(cache.manifest().unwrap(), Manifest::default());

        let mut manifest = Manifest::default();
        manifest.scryfall.insert(
            "oracle_cards".into(),
            BulkRecord {
                file: record("scryfall/oracle.jsonl.gz", 5),
                updated_at: "2026-10-18T21:02:00.694+00:00".into(),
            },
        );
        manifest.mtgjson.index = Some(IndexRecord {
            file: record("mtgjson/DeckList.json", 9),
            build: Some("5.3.0+20261018".into()),
            validators: Validators {
                etag: Some("W/\"1\"".into()),
                last_modified: None,
            },
        });
        manifest.mtgjson.decks.insert(
            "Deck_ABC".into(),
            DeckRecord {
                file: record("mtgjson/decks/Deck_ABC.json", 3),
                build: None,
                validators: Validators::default(),
            },
        );
        cache.save(&manifest).unwrap();

        assert_eq!(cache.manifest().unwrap(), manifest);
    }

    #[test]
    fn an_unreadable_manifest_is_an_error_not_an_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(MANIFEST), "{not json").unwrap();
        assert!(matches!(
            Cache::new(dir.path()).manifest(),
            Err(CliError::Cache { .. })
        ));
    }

    #[test]
    fn a_record_counts_only_while_its_file_has_the_recorded_length() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path());
        cache.prepare("scryfall").unwrap();
        let target = cache.path("scryfall/f.gz");
        assert!(!cache.holds(&record("scryfall/f.gz", 3)));
        fs::write(&target, b"abc").unwrap();
        assert!(cache.holds(&record("scryfall/f.gz", 3)));
        assert!(!cache.holds(&record("scryfall/f.gz", 4)));
        assert!(!cache.holds(&record("scryfall", 0)));
    }
}
