//! Drafts of the read tier in this crate: the whole rules files under `drafts/` that `promote --draft` writes, embedded
//! at build time; whether one fits its stored card, the smoke gate on one, and the store `compile` keeps in `card_drafts`
//! (`docs/cards-dsl.md`, Model-assisted reading, Drafts).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mtg_cards::card::Card;
use mtg_cards::compile::compile;
use mtg_cards::load::{Library, LoadError};
use mtg_cards::support::{Support, read_hash, support};
use mtg_cards::{Caps, CardDefinition, DraftRules, PrintedCard, ReadState};
use mtg_db::{Database, DraftKey, NewDraft};
use mtg_import::ids::OracleId;
use mtg_testkit::smoke::{DEFAULT_SEEDS, SmokeFailure, SmokeReport, smoke_card};

use crate::compile::Invalid;
use crate::error::CliError;
use crate::status::missing_table;

/// Every file under `drafts/` with its slug (the file name without `.json`), by slug.
const FILES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/draft_files.rs"));

/// Where `promote --draft` writes, seen from the repository root.
pub const DEFAULT_DIR: &str = "crates/mtg-cli/drafts";

/// The folder of the cards' Scryfall records below the drafts directory.
pub const FIXTURES: &str = "fixtures";

/// How many drafts the compile report lists as invalid.
const INVALID_SHOWN: usize = 20;

/// The embedded draft files: slug and text, by slug.
pub fn embedded() -> &'static [(&'static str, &'static str)] {
    FILES
}

/// Why a draft does not stand for its stored card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Misfit {
    /// The rules fail a check of the loader or are not whole (a status other than `full`, a `Manual` ability).
    Unloadable(String),
    /// Written for another text: a face's name or fingerprint differs from the stored card's.
    Stale,
    /// A rules file or a `Verified` compiled row already runs the card.
    Outranked,
}

impl Misfit {
    /// The reason in a few words.
    pub fn reason(&self) -> String {
        match self {
            Misfit::Unloadable(message) => format!("not a loadable whole draft: {message}"),
            Misfit::Stale => "written for another text".into(),
            Misfit::Outranked => "the card is Verified without it".into(),
        }
    }
}

/// The row a library takes for a draft, with the read state it should have.
fn row(printed: &PrintedCard, card: &Card, read: ReadState) -> DraftRules {
    DraftRules {
        oracle_id: printed.oracle_id.clone(),
        read_hash: read_hash(printed, card),
        rules: serde_json::to_value(card).unwrap_or_default(),
        read,
    }
}

/// The first problem of each row the library left out, by oracle id.
fn broken_rows(left_out: &[LoadError]) -> BTreeMap<&str, String> {
    let mut broken = BTreeMap::new();
    for error in left_out {
        if let Some(id) = error.file.strip_prefix("card_drafts/") {
            broken
                .entry(id)
                .or_insert_with(|| format!("{}: {}", error.path, error.message));
        }
    }
    broken
}

/// Whether each draft fits its stored card, in the order of `items`: it loads as a draft (a whole rules file), is
/// written for the stored text, and no rules file or `Verified` compiled row runs the card without it.
pub fn misfits(
    library: &Library,
    caps: &Caps,
    items: &[(&PrintedCard, &Card)],
) -> Vec<Option<Misfit>> {
    let rows: Vec<DraftRules> = items
        .iter()
        .map(|(printed, card)| row(printed, card, ReadState::Pending))
        .collect();
    let (_, left_out) = library.with_drafts(&rows);
    let broken = broken_rows(&left_out);
    items
        .iter()
        .map(|(printed, card)| {
            if let Some(message) = broken.get(printed.oracle_id.as_str()) {
                return Some(Misfit::Unloadable(message.clone()));
            }
            if support(Some(card), printed, caps).support == Support::Stale {
                return Some(Misfit::Stale);
            }
            let compiled = compile(printed).card;
            let verified = support(compiled.as_ref(), printed, caps).support == Support::Verified;
            (verified || library.card(&printed.oracle_id).is_some()).then_some(Misfit::Outranked)
        })
        .collect()
}

/// The definition the engine runs for each draft once a player approved it, in the order of `items`, or why the engine
/// would not run it as `Read` (the loader refuses it, a primitive it lacks, a card it knows better).
pub fn read_definitions(
    library: &Library,
    caps: &Caps,
    items: &[(&PrintedCard, &Card)],
) -> Vec<Result<CardDefinition, String>> {
    let rows: Vec<DraftRules> = items
        .iter()
        .map(|(printed, card)| row(printed, card, ReadState::Approved))
        .collect();
    let (library, left_out) = library.with_drafts(&rows);
    let broken = broken_rows(&left_out);
    items
        .iter()
        .map(|(printed, _)| {
            if let Some(message) = broken.get(printed.oracle_id.as_str()) {
                return Err(message.clone());
            }
            let assembled = library.assemble(printed, caps);
            match assembled.support.support {
                Support::Read => Ok(assembled.definition),
                state => Err(format!("the engine runs it as {state:?}, not Read")),
            }
        })
        .collect()
}

/// How a draft failed the smoke gate, without the game it failed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmokeKind {
    /// An invariant of the engine broke.
    Invariant,
    /// The engine refused an answer or failed internally.
    Engine,
    /// A game did not end.
    Stuck,
    /// A game ended in an inconsistent way.
    BadEnding,
    /// The engine panicked.
    Panicked,
    /// No game used the card.
    NeverSeen,
}

impl SmokeKind {
    /// The kind of a failure.
    pub fn of(failure: &SmokeFailure) -> SmokeKind {
        match failure {
            SmokeFailure::Invariant { .. } => SmokeKind::Invariant,
            SmokeFailure::Engine { .. } => SmokeKind::Engine,
            SmokeFailure::Stuck { .. } => SmokeKind::Stuck,
            SmokeFailure::BadEnding { .. } => SmokeKind::BadEnding,
            SmokeFailure::Panicked { .. } => SmokeKind::Panicked,
            SmokeFailure::NeverSeen { .. } => SmokeKind::NeverSeen,
        }
    }

    /// The kind in a word.
    pub fn word(self) -> &'static str {
        match self {
            SmokeKind::Invariant => "invariant",
            SmokeKind::Engine => "engine error",
            SmokeKind::Stuck => "stuck",
            SmokeKind::BadEnding => "bad ending",
            SmokeKind::Panicked => "panic",
            SmokeKind::NeverSeen => "never seen",
        }
    }
}

/// Plays the draft in the bot games of the gate (the default seeds, at two and four seats): the failure's kind and its
/// first game, or the report of a draft that passes.
pub fn smoke(definition: &CardDefinition) -> Result<SmokeReport, (SmokeKind, String)> {
    let report = smoke_card(definition, &DEFAULT_SEEDS);
    match &report.failure {
        Some(failure) => Err((SmokeKind::of(failure), failure.to_string())),
        None => Ok(report),
    }
}

/// What `compile` did with the drafts the build embeds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DraftCounts {
    /// Draft files in the build.
    pub embedded: usize,
    /// Drafts stored that were not before.
    pub new: usize,
    /// Drafts stored and retired that are current again.
    pub revived: usize,
    /// Drafts that were stored and current already.
    pub unchanged: usize,
    /// Stored current drafts the build no longer holds, now retired.
    pub retired: usize,
    /// Drafts of a card that is not stored.
    pub no_card: usize,
    /// Drafts written for another text than the stored card's.
    pub stale: usize,
    /// Drafts of a card that is `Verified` without them.
    pub outranked: usize,
    /// Drafts that do not parse or load as a whole draft.
    pub invalid: Vec<Invalid>,
}

impl DraftCounts {
    /// The counts as text.
    pub fn render(&self) -> String {
        let mut text = format!(
            "  drafts: {} in the build: {} new, {} revived, {} unchanged, {} retired\n  not stored: {} no stored card, {} written for another text, {} Verified without them, {} invalid",
            self.embedded,
            self.new,
            self.revived,
            self.unchanged,
            self.retired,
            self.no_card,
            self.stale,
            self.outranked,
            self.invalid.len()
        );
        for card in self.invalid.iter().take(INVALID_SHOWN) {
            let _ = write!(text, "\n    {}: {}", card.card, card.problems.join("; "));
        }
        if self.invalid.len() > INVALID_SHOWN {
            let _ = write!(
                text,
                "\n    and {} more",
                self.invalid.len() - INVALID_SHOWN
            );
        }
        text
    }
}

/// The drafts of the build that fit a stored card, as rows to store, with the counts of those left out.
fn fitting(
    files: &[(&str, &str)],
    library: &Library,
    caps: &Caps,
    cards: &[PrintedCard],
) -> (Vec<NewDraft>, DraftCounts) {
    let by_id: BTreeMap<&str, &PrintedCard> = cards
        .iter()
        .map(|card| (card.oracle_id.as_str(), card))
        .collect();
    let mut counts = DraftCounts {
        embedded: files.len(),
        ..DraftCounts::default()
    };
    let mut parsed: Vec<(&PrintedCard, Card, &str)> = Vec::new();
    for (slug, text) in files {
        match serde_json::from_str::<Card>(text) {
            Err(error) => counts.invalid.push(Invalid {
                card: (*slug).to_owned(),
                problems: vec![error.to_string()],
            }),
            Ok(card) => match by_id.get(card.oracle_id.as_str()) {
                Some(printed) => parsed.push((printed, card, slug)),
                None => counts.no_card += 1,
            },
        }
    }
    let items: Vec<(&PrintedCard, &Card)> = parsed.iter().map(|(p, c, _)| (*p, c)).collect();
    let mut put = BTreeMap::new();
    for ((printed, card, slug), misfit) in parsed.iter().zip(misfits(library, caps, &items)) {
        match misfit {
            Some(Misfit::Stale) => counts.stale += 1,
            Some(Misfit::Outranked) => counts.outranked += 1,
            Some(Misfit::Unloadable(message)) => counts.invalid.push(Invalid {
                card: (*slug).to_owned(),
                problems: vec![message],
            }),
            None => {
                let draft = NewDraft {
                    oracle_id: OracleId(printed.oracle_id.clone()),
                    read_hash: read_hash(printed, card),
                    rules: serde_json::to_value(card).unwrap_or_default(),
                };
                put.insert(draft.oracle_id.clone(), draft);
            }
        }
    }
    (put.into_values().collect(), counts)
}

/// What a run writes to `card_drafts`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DraftChange {
    /// Drafts to make current: new ones and retired ones that return.
    pub put: Vec<NewDraft>,
    /// Stored current drafts the build does not hold.
    pub retire: Vec<DraftKey>,
}

/// Sorts the drafts the build holds against the stored ones: `stored` maps each key already stored (retired or not) to
/// whether it is retired, `current` lists the keys that are not retired. Fills the counts of the sorting.
pub fn plan_changes(
    held: Vec<NewDraft>,
    stored: &BTreeMap<DraftKey, bool>,
    current: &[DraftKey],
    counts: &mut DraftCounts,
) -> DraftChange {
    let mut change = DraftChange::default();
    let mut keys = BTreeSet::new();
    for draft in held {
        let key = DraftKey {
            oracle_id: draft.oracle_id.clone(),
            read_hash: draft.read_hash.clone(),
        };
        match stored.get(&key) {
            Some(false) => counts.unchanged += 1,
            Some(true) => {
                counts.revived += 1;
                change.put.push(draft);
            }
            None => {
                counts.new += 1;
                change.put.push(draft);
            }
        }
        keys.insert(key);
    }
    change.retire = current
        .iter()
        .filter(|key| !keys.contains(*key))
        .cloned()
        .collect();
    counts.retired = change.retire.len();
    change
}

/// Makes the drafts of the build that fit the stored `cards` the current drafts in `card_drafts` (the card's other
/// current draft is retired, a retired draft of the same hash is current again) and retires the stored current drafts the
/// build no longer holds. Rows are never deleted.
pub async fn store(
    db: &Database,
    library: &Library,
    caps: &Caps,
    cards: &[PrintedCard],
) -> Result<DraftCounts, CliError> {
    store_files(db, embedded(), library, caps, cards).await
}

/// [`store`] for these draft files (slug and text) instead of the embedded ones.
pub async fn store_files(
    db: &Database,
    files: &[(&str, &str)],
    library: &Library,
    caps: &Caps,
    cards: &[PrintedCard],
) -> Result<DraftCounts, CliError> {
    let (held, mut counts) = fitting(files, library, caps, cards);
    let repo = db.drafts();
    let missing = |error: mtg_db::DbError| {
        if missing_table(&error) {
            CliError::Config("the schema is not applied yet; `mtg-cli ingest` applies it".into())
        } else {
            CliError::Db(error)
        }
    };
    let keys: Vec<DraftKey> = held
        .iter()
        .map(|draft| DraftKey {
            oracle_id: draft.oracle_id.clone(),
            read_hash: draft.read_hash.clone(),
        })
        .collect();
    let stored: BTreeMap<DraftKey, bool> = repo
        .get(&keys)
        .await
        .map_err(missing)?
        .iter()
        .map(|draft| (draft.key(), draft.retired))
        .collect();
    let current: Vec<DraftKey> = repo
        .current()
        .await?
        .iter()
        .map(mtg_db::StoredDraft::key)
        .collect();
    let change = plan_changes(held, &stored, &current, &mut counts);
    repo.insert(&change.put).await?;
    repo.retire(&change.retire).await?;
    Ok(counts)
}

#[cfg(test)]
mod tests {
    //! Goal: the build's drafts sort against the stored ones into new, revived and unchanged, a stored current draft
    //! the build lacks is retired, and a card that appears in two ways keeps one draft.
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    fn draft(id: &str, hash: &str) -> NewDraft {
        NewDraft {
            oracle_id: OracleId(id.into()),
            read_hash: hash.into(),
            rules: json!({}),
        }
    }

    fn key(id: &str, hash: &str) -> DraftKey {
        DraftKey {
            oracle_id: OracleId(id.into()),
            read_hash: hash.into(),
        }
    }

    /// Counts as (new, revived, unchanged, retired) and the keys put and retired.
    type Outcome = (
        (usize, usize, usize, usize),
        Vec<&'static str>,
        Vec<&'static str>,
    );

    #[rstest]
    #[case::empty(&[], &[], ((0, 0, 0, 0), vec![], vec![]))]
    #[case::new(&[("a", "h1")], &[], ((1, 0, 0, 0), vec!["h1"], vec![]))]
    #[case::unchanged(&[("a", "h1")], &[("a", "h1", false)], ((0, 0, 1, 0), vec![], vec![]))]
    #[case::revived(&[("a", "h1")], &[("a", "h1", true), ("a", "h2", false)], ((0, 1, 0, 1), vec!["h1"], vec!["h2"]))]
    #[case::replaced(&[("a", "h2")], &[("a", "h1", false)], ((1, 0, 0, 1), vec!["h2"], vec!["h1"]))]
    #[case::gone(&[], &[("a", "h1", false)], ((0, 0, 0, 1), vec![], vec!["h1"]))]
    fn the_build_is_sorted_against_the_stored_drafts(
        #[case] held: &[(&str, &str)],
        #[case] stored: &[(&str, &str, bool)],
        #[case] expected: Outcome,
    ) {
        let held: Vec<NewDraft> = held.iter().map(|(id, hash)| draft(id, hash)).collect();
        let stored_map: BTreeMap<DraftKey, bool> = stored
            .iter()
            .map(|(id, hash, retired)| (key(id, hash), *retired))
            .collect();
        let current: Vec<DraftKey> = stored
            .iter()
            .filter(|(_, _, retired)| !retired)
            .map(|(id, hash, _)| key(id, hash))
            .collect();
        let mut counts = DraftCounts::default();

        let change = plan_changes(held, &stored_map, &current, &mut counts);

        let counted = (counts.new, counts.revived, counts.unchanged, counts.retired);
        let put: Vec<&str> = change.put.iter().map(|d| d.read_hash.as_str()).collect();
        let retire: Vec<&str> = change.retire.iter().map(|k| k.read_hash.as_str()).collect();
        assert_eq!((counted, put, retire), (expected.0, expected.1, expected.2));
    }

    #[test]
    fn the_counts_render_every_figure() {
        let counts = DraftCounts {
            embedded: 9,
            new: 3,
            revived: 1,
            unchanged: 2,
            retired: 4,
            no_card: 5,
            stale: 6,
            outranked: 7,
            invalid: vec![Invalid {
                card: "x".into(),
                problems: vec!["bad".into()],
            }],
        };
        let text = counts.render();
        for part in [
            "9 in the build",
            "3 new",
            "1 revived",
            "2 unchanged",
            "4 retired",
            "5 no stored card",
            "6 written for another text",
            "7 Verified without them",
            "1 invalid",
            "x: bad",
        ] {
            assert!(text.contains(part), "{part} in {text}");
        }
    }
}
