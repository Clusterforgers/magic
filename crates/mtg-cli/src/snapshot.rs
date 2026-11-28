//! `coverage --all --snapshot`: how many Commander-legal cards play automatically, stored as a coverage snapshot for the
//! Card support page: the counts per state, the popularity buckets, every card that does not play automatically with
//! its first blocker in plain words, and the cards that play automatically since the snapshot before. Popularity comes
//! from the stored ranks, so it needs no cache and runs beside the compile job of a release.

use std::collections::{BTreeMap, HashMap, HashSet};

use mtg_cards::coverage::{Blocker, Bucket, CardResult, PreconCard, Report, analyze_with};
use mtg_db::{CardSearch, Database, MissingCard, NewSnapshot, RankBucket, StateCounts, Stored};
use mtg_import::ids::OracleId;

use crate::audit::stored_cards_matching;
use crate::error::CliError;
use crate::reads::draft_library;

/// The rank buckets: the most played cards of the pool up to each size, as in `coverage --all`.
const RANK_BUCKETS: [usize; 5] = [100, 500, 1_000, 2_000, 5_000];

/// What `coverage --all --snapshot` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotOutcome {
    /// Whether a row was written or the newest one already said the same.
    pub stored: Stored,
    /// Commander-legal cards judged.
    pub cards: u32,
    /// Of them, the cards that play automatically.
    pub automatic: u32,
    /// How many play automatically since the snapshot before.
    pub newly_supported: u32,
}

impl SnapshotOutcome {
    /// The one-line summary.
    pub fn summary(&self) -> String {
        let share = if self.cards == 0 {
            0.0
        } else {
            100.0 * f64::from(self.automatic) / f64::from(self.cards)
        };
        let what = match self.stored {
            Stored::New(id) => format!("stored as #{id}"),
            Stored::Unchanged(id) => format!("unchanged since #{id}"),
        };
        format!(
            "coverage snapshot: {} of {} Commander cards play automatically ({share:.1}%), {} newly, {what}",
            self.automatic, self.cards, self.newly_supported
        )
    }
}

/// Whether a card of this bucket plays without any clause by hand.
const fn is_automatic(bucket: Bucket) -> bool {
    matches!(bucket, Bucket::Verified | Bucket::Read | Bucket::Unverified)
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Judges every stored Commander-legal card and stores the snapshot unless it equals the newest one.
pub async fn take_snapshot(
    db: &Database,
    release: Option<&str>,
) -> Result<SnapshotOutcome, CliError> {
    let search = CardSearch {
        legal_in: Some("commander".to_owned()),
        ..CardSearch::default()
    };
    let printed = stored_cards_matching(db, &search).await?;
    if printed.is_empty() {
        return Err(CliError::Config(
            "no stored card is legal in Commander; `mtg-cli fetch scryfall` and `mtg-cli ingest` load them"
                .into(),
        ));
    }
    let cards: Vec<PreconCard> = printed
        .into_iter()
        .map(|printed| PreconCard {
            printed,
            copies: BTreeMap::new(),
        })
        .collect();
    let (library, _skipped) = draft_library(db).await?;
    let report = analyze_with(&cards, &library, &mtg_engine::caps());
    let ids: Vec<OracleId> = report
        .cards
        .iter()
        .map(|card| OracleId(card.oracle_id.clone()))
        .collect();
    let ranks = db.ranks().of(&ids).await?;
    let ranks: HashMap<String, u32> = ranks.into_iter().map(|(id, rank)| (id.0, rank)).collect();
    let feed = db.card_feed();
    let previous = feed.latest_missing().await?.map(|(_, cards)| cards);
    let snapshot = snapshot_of(&report, &ranks, release, previous.as_deref());
    let stored = feed.store_snapshot(&snapshot).await?;
    Ok(SnapshotOutcome {
        stored,
        cards: snapshot.cards,
        automatic: snapshot.automatic,
        newly_supported: match stored {
            Stored::New(_) => snapshot.newly_supported_count,
            Stored::Unchanged(_) => 0,
        },
    })
}

/// The snapshot of `report`: `ranks` order the cards (the unranked last, by name) and `previous`, the missing cards of
/// the newest stored snapshot, gives the cards that play automatically since.
pub fn snapshot_of<S: std::hash::BuildHasher>(
    report: &Report,
    ranks: &HashMap<String, u32, S>,
    release: Option<&str>,
    previous: Option<&[MissingCard]>,
) -> NewSnapshot {
    let mut ordered: Vec<(&CardResult, Option<u32>)> = report
        .cards
        .iter()
        .map(|card| (card, ranks.get(&card.oracle_id).copied()))
        .collect();
    ordered.sort_by(|a, b| (a.1.is_none(), a.1, &a.0.name).cmp(&(b.1.is_none(), b.1, &b.0.name)));
    let mut states = StateCounts::default();
    for (card, _) in &ordered {
        let slot = match card.bucket() {
            Bucket::Verified => &mut states.verified,
            Bucket::Read => &mut states.read,
            Bucket::Unverified => &mut states.unverified,
            Bucket::Waiting => &mut states.waiting,
            Bucket::Partial => &mut states.partial,
            Bucket::Manual => &mut states.manual,
            Bucket::Other => &mut states.other,
        };
        *slot += 1;
    }
    let missing: Vec<MissingCard> = ordered
        .iter()
        .filter(|(card, _)| !is_automatic(card.bucket()))
        .map(|(card, rank)| missing_card(card, *rank))
        .collect();
    let automatic = ordered
        .iter()
        .filter(|(card, _)| is_automatic(card.bucket()))
        .count();
    let newly_supported = newly_supported(report, previous);
    NewSnapshot {
        release: release.filter(|r| !r.is_empty()).map(str::to_owned),
        cards: u32_of(ordered.len()),
        automatic: u32_of(automatic),
        states,
        buckets: buckets(&ordered),
        missing,
        newly_supported_count: u32_of(newly_supported.len()),
        newly_supported,
    }
}

/// The names of the cards of `previous` that `report` holds and plays automatically now, in the order of `previous`.
fn newly_supported(report: &Report, previous: Option<&[MissingCard]>) -> Vec<String> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let automatic: HashSet<&str> = report
        .cards
        .iter()
        .filter(|card| is_automatic(card.bucket()))
        .map(|card| card.oracle_id.as_str())
        .collect();
    previous
        .iter()
        .filter(|card| automatic.contains(card.oracle_id.as_str()))
        .map(|card| card.name.clone())
        .collect()
}

fn buckets(ordered: &[(&CardResult, Option<u32>)]) -> Vec<RankBucket> {
    let ranked = ordered.partition_point(|(_, rank)| rank.is_some());
    let bucket = |label: String, part: &[(&CardResult, Option<u32>)]| RankBucket {
        label,
        cards: u32_of(part.len()),
        verified: u32_of(
            part.iter()
                .filter(|(card, _)| card.bucket() == Bucket::Verified)
                .count(),
        ),
        automatic: u32_of(
            part.iter()
                .filter(|(card, _)| is_automatic(card.bucket()))
                .count(),
        ),
    };
    let mut all: Vec<RankBucket> = RANK_BUCKETS
        .iter()
        .filter(|&&size| size <= ranked)
        .map(|&size| bucket(format!("top {size}"), &ordered[..size]))
        .collect();
    all.push(bucket("all ranked".to_owned(), &ordered[..ranked]));
    all.push(bucket("no rank".to_owned(), &ordered[ranked..]));
    all
}

fn missing_card(card: &CardResult, rank: Option<u32>) -> MissingCard {
    let first = card.blockers.iter().next();
    let detail = first.map_or_default(Blocker::label);
    let blocker = if card.waiting {
        "Waits for a player to read its rules".to_owned()
    } else {
        first.map_or_else(|| "No rules yet".to_owned(), plain_words)
    };
    MissingCard {
        oracle_id: card.oracle_id.clone(),
        name: card.name.clone(),
        state: card.bucket().name().to_owned(),
        blocker,
        detail,
        blockers: u32_of(card.blockers.len()),
        rank,
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.trim().chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// `Effect.LoseGame` as `lose game`.
fn primitive_words(name: &str) -> String {
    let last = name.rsplit('.').next().unwrap_or(name);
    let mut words = String::new();
    for (at, c) in last.chars().enumerate() {
        if c.is_uppercase() && at > 0 {
            words.push(' ');
        }
        words.extend(c.to_lowercase());
    }
    words
}

/// A blocker in the words a player reads.
pub fn plain_words(blocker: &Blocker) -> String {
    match blocker {
        Blocker::Clause(shape) => match shape.split_once(": ") {
            Some(("manual", reason)) => capitalized(reason),
            Some((kind, _)) => {
                let what = match kind {
                    "keyword" => "A keyword",
                    "spell" => "Part of the spell's text",
                    "triggered" => "A triggered ability",
                    "activated" => "An activated ability",
                    "loyalty" => "A loyalty ability",
                    "modal" => "A choice of modes",
                    _ => "A static ability",
                };
                format!("{what} the game cannot read yet")
            }
            None => capitalized(shape),
        },
        Blocker::Primitive(name) => {
            format!(
                "The engine does not run \u{201c}{}\u{201d} yet",
                primitive_words(name)
            )
        }
        Blocker::Printed(what) => capitalized(what),
        Blocker::Rules(what) => match what.as_str() {
            "invalid" => "Its rules do not pass the checks".to_owned(),
            "stale" => "Its Oracle text changed since its rules were written".to_owned(),
            "unplayable" => "The engine cannot host this card".to_owned(),
            "no rules" => "No rules yet".to_owned(),
            mtg_cards::support::READBACK_DIFFERS => {
                "Its rules read back as other text than the card".to_owned()
            }
            other => capitalized(other),
        },
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a snapshot counts the cards per state and per popularity bucket, lists every card that does not play automatically in rank order with its first blocker in plain words, and names the cards that play automatically since the snapshot before.
    #![allow(clippy::unwrap_used)]

    use std::collections::BTreeSet;

    use mtg_cards::coverage::RulesSource;
    use mtg_cards::support::Support;
    use rstest::rstest;

    use super::*;

    fn card(id: &str, support: Support, waiting: bool, blockers: &[Blocker]) -> CardResult {
        CardResult {
            oracle_id: id.to_owned(),
            name: format!("Card {id}"),
            rules: RulesSource::Compiled,
            support,
            blockers: blockers.iter().cloned().collect::<BTreeSet<_>>(),
            waiting,
            copies: BTreeMap::new(),
        }
    }

    fn clause(shape: &str) -> Blocker {
        Blocker::Clause(shape.to_owned())
    }

    fn report() -> Report {
        Report {
            cards: vec![
                card("a", Support::Verified, false, &[]),
                card(
                    "b",
                    Support::Partial,
                    false,
                    &[clause("triggered: When ~ dies, N")],
                ),
                card("c", Support::Unverified, false, &[]),
                card(
                    "d",
                    Support::Manual,
                    true,
                    &[clause("static: ~ can't block")],
                ),
                card(
                    "e",
                    Support::Stale,
                    false,
                    &[Blocker::Rules("stale".into())],
                ),
                card("f", Support::Read, false, &[]),
            ],
        }
    }

    fn ranks() -> HashMap<String, u32> {
        [("a", 3), ("b", 1), ("c", 2), ("d", 4)]
            .into_iter()
            .map(|(id, rank)| (id.to_owned(), rank))
            .collect()
    }

    #[test]
    fn the_snapshot_counts_states_and_lists_the_missing_in_rank_order() {
        let snapshot = snapshot_of(&report(), &ranks(), Some("v0.2.9"), None);
        assert_eq!((snapshot.cards, snapshot.automatic), (6, 3));
        assert_eq!(
            snapshot.states,
            StateCounts {
                verified: 1,
                read: 1,
                unverified: 1,
                waiting: 1,
                partial: 1,
                manual: 0,
                other: 1,
            }
        );
        let missing: Vec<(&str, &str, Option<u32>)> = snapshot
            .missing
            .iter()
            .map(|card| (card.oracle_id.as_str(), card.state.as_str(), card.rank))
            .collect();
        assert_eq!(
            missing,
            vec![
                ("b", "partial", Some(1)),
                ("d", "waiting", Some(4)),
                ("e", "other", None)
            ]
        );
        assert_eq!(
            snapshot.missing[1].blocker,
            "Waits for a player to read its rules"
        );
        assert_eq!(snapshot.missing[1].detail, "static: ~ can't block");
        assert_eq!(snapshot.release.as_deref(), Some("v0.2.9"));
        assert_eq!(snapshot.newly_supported, Vec::<String>::new());
    }

    #[test]
    fn buckets_count_only_the_sizes_the_ranked_cards_fill() {
        let snapshot = snapshot_of(&report(), &ranks(), None, None);
        let rows: Vec<(&str, u32, u32, u32)> = snapshot
            .buckets
            .iter()
            .map(|b| (b.label.as_str(), b.cards, b.verified, b.automatic))
            .collect();
        assert_eq!(rows, vec![("all ranked", 4, 1, 2), ("no rank", 2, 0, 1)]);
    }

    #[test]
    fn newly_supported_cards_were_missing_and_play_automatically_now() {
        let before = snapshot_of(&report(), &ranks(), None, None);
        let mut now = report();
        now.cards[1].support = Support::Verified;
        now.cards[1].blockers.clear();
        now.cards[3].waiting = false;
        let after = snapshot_of(&now, &ranks(), None, Some(&before.missing));
        assert_eq!(after.newly_supported, vec!["Card b".to_owned()]);
        assert_eq!(after.newly_supported_count, 1);
        assert_eq!(after.automatic, 4);
    }

    #[rstest]
    #[case(
        clause("triggered: When ~ dies, N"),
        "A triggered ability the game cannot read yet"
    )]
    #[case(
        clause("activated: {M}, {T}: draw N"),
        "An activated ability the game cannot read yet"
    )]
    #[case(
        clause("static: ~ can't block"),
        "A static ability the game cannot read yet"
    )]
    #[case(clause("manual: you vote by hand"), "You vote by hand")]
    #[case(Blocker::Primitive("Effect.LoseGame".into()), "The engine does not run \u{201c}lose game\u{201d} yet")]
    #[case(Blocker::Printed("an {X} cost".into()), "An {X} cost")]
    #[case(Blocker::Rules("invalid".into()), "Its rules do not pass the checks")]
    #[case(Blocker::Rules("no rules".into()), "No rules yet")]
    #[case(Blocker::Rules("unplayable".into()), "The engine cannot host this card")]
    #[case(Blocker::Rules("readback:Differs".into()), "Its rules read back as other text than the card")]
    fn blockers_read_in_plain_words(#[case] blocker: Blocker, #[case] words: &str) {
        assert_eq!(plain_words(&blocker), words);
    }

    #[test]
    fn the_summary_says_the_share_and_what_was_stored() {
        let outcome = SnapshotOutcome {
            stored: Stored::Unchanged(4),
            cards: 200,
            automatic: 150,
            newly_supported: 0,
        };
        assert_eq!(
            outcome.summary(),
            "coverage snapshot: 150 of 200 Commander cards play automatically (75.0%), 0 newly, unchanged since #4"
        );
    }
}
