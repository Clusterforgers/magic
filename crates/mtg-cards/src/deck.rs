//! The deck check: whether a deck is legal in its format, how far the game plays each of its cards,
//! and the two hashes the Verified marker is derived from. One pure function, [`deck_report`], serves
//! the server, the CLI and the tests (`docs/deck-check.md`).
//!
//! The report keeps "illegal" (`legality`) apart from "not implemented" (`cards`), reports every
//! zone, and counts the sideboard and the companion like any other card when it decides the state.

mod companion;
mod hashes;
mod legality;
mod marker;

use std::collections::BTreeMap;

use mtg_core::color::ColorSet;
use serde::{Deserialize, Serialize};

use crate::load::Library;
use crate::support::{Caps, PrintedCard, Reason, Support};

pub use hashes::{coverage_hash, deck_hash};
pub use legality::{Issue, IssueCode, SUPPORTED_FORMATS, is_supported_format};
pub use marker::{Marker, StoredCheck, marker};

/// A part of a deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum DeckZone {
    /// The main deck.
    Main,
    /// The sideboard.
    Sideboard,
    /// The command zone: the commanders of a Commander deck.
    Commander,
    /// The companion.
    Companion,
}

impl DeckZone {
    /// Every zone, in the order a report lists them.
    pub const ALL: [DeckZone; 4] = [
        DeckZone::Main,
        DeckZone::Sideboard,
        DeckZone::Commander,
        DeckZone::Companion,
    ];

    /// The zone's name on the wire and in the hash.
    pub const fn keyword(self) -> &'static str {
        match self {
            DeckZone::Main => "main",
            DeckZone::Sideboard => "sideboard",
            DeckZone::Commander => "commander",
            DeckZone::Companion => "companion",
        }
    }
}

/// Copies of one card in one zone. Printings are not part of a deck for the check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckLine {
    /// The zone.
    pub zone: DeckZone,
    /// The oracle id in its canonical lower-case, hyphenated form.
    pub oracle_id: String,
    /// How many copies.
    pub count: u32,
}

/// Scryfall's verdict on a card in one format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Legality {
    /// Legal.
    Legal,
    /// Not legal (not in the format's card pool).
    NotLegal,
    /// Legal, but at most one copy.
    Restricted,
    /// Banned.
    Banned,
    /// A keyword this crate does not know.
    Unknown,
}

impl Legality {
    /// Reads a Scryfall legality keyword (`legal`, `not_legal`, `restricted`, `banned`).
    pub fn parse(keyword: &str) -> Legality {
        match keyword {
            "legal" => Legality::Legal,
            "not_legal" => Legality::NotLegal,
            "restricted" => Legality::Restricted,
            "banned" => Legality::Banned,
            _ => Legality::Unknown,
        }
    }
}

/// What the check needs to know of one oracle card besides its rules file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardFacts {
    /// The imported card: names, type lines, text and keywords; also what support is judged against.
    pub printed: PrintedCard,
    /// The card's color identity.
    pub color_identity: ColorSet,
    /// Scryfall's legality per format, by the format's lower-case key.
    pub legalities: BTreeMap<String, Legality>,
}

/// How many copies of a deck's cards are in each [`Support`] state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Counts {
    /// All copies.
    pub total: u32,
    /// Copies of `verified` cards.
    pub verified: u32,
    /// Copies of `partial` cards.
    pub partial: u32,
    /// Copies of `manual` cards.
    pub manual: u32,
    /// Copies of `stale` cards, which count as manual until reviewed.
    pub stale: u32,
    /// Copies of `unplayable` cards.
    pub unplayable: u32,
}

impl Counts {
    fn add(&mut self, support: Support, copies: u32) {
        self.total = self.total.saturating_add(copies);
        let slot = match support {
            Support::Verified => &mut self.verified,
            Support::Partial => &mut self.partial,
            Support::Manual => &mut self.manual,
            Support::Stale => &mut self.stale,
            Support::Unplayable => &mut self.unplayable,
        };
        *slot = slot.saturating_add(copies);
    }
}

/// The counts of one zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ZoneCounts {
    /// The zone.
    pub zone: DeckZone,
    /// Its copies by support state.
    pub counts: Counts,
}

/// What a deck is, as far as the table is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum DeckState {
    /// Legal, and every card of every zone is `verified`.
    Verified,
    /// Legal, with some cards that are not `verified`; the table resolves them by hand.
    ManualPlay,
    /// Illegal in its format, or holds an `unplayable` card.
    Blocked,
    /// Never checked, or changed since: a report is never in this state.
    Unchecked,
}

impl DeckState {
    /// How bad the state is for playing: `verified` < `manual_play` < `blocked`; `unchecked` has no rank.
    const fn rank(self) -> Option<u8> {
        match self {
            DeckState::Verified => Some(0),
            DeckState::ManualPlay => Some(1),
            DeckState::Blocked => Some(2),
            DeckState::Unchecked => None,
        }
    }
}

/// The counts of a report and the state they give.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Summary {
    /// The format the deck was checked in.
    pub format: String,
    /// `blocked` if the deck is illegal or holds an unplayable card, `manual_play` if some card is
    /// not verified, otherwise `verified`.
    pub state: DeckState,
    /// Copies by support state across all zones.
    pub counts: Counts,
    /// The same per zone, for the zones that hold cards.
    pub zones: Vec<ZoneCounts>,
    /// How many legality issues the deck has.
    pub issues: u32,
}

/// One card of a deck as the report shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReportCard {
    /// The oracle id.
    pub oracle_id: String,
    /// The card's name; the oracle id for a card the database does not hold.
    pub name: String,
    /// The zone this line sits in.
    pub zone: DeckZone,
    /// Copies in that zone.
    pub count: u32,
    /// How far the game plays the card.
    pub support: Support,
    /// The clauses left to the table, each with the span of oracle text it covers.
    pub reasons: Vec<Reason>,
}

/// The result of checking a deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeckReport {
    /// SHA-256 over the deck's `(zone, oracle id, count)`; see [`deck_hash`].
    pub deck_hash: String,
    /// SHA-256 over the `(oracle id, support fingerprint)` of its cards; see [`coverage_hash`].
    pub coverage_hash: String,
    /// The format the deck was checked in.
    pub format: String,
    /// Why the deck is not legal in its format; empty if it is.
    pub legality: Vec<Issue>,
    /// The cards by zone (main, sideboard, commander, companion), each zone by name.
    pub cards: Vec<ReportCard>,
    /// The counts and the state.
    pub summary: Summary,
}

impl DeckReport {
    /// Whether the deck is legal and every card of it is `verified`.
    pub fn is_clean(&self) -> bool {
        self.summary.state == DeckState::Verified
    }

    /// Replaces the legality, as a table's rules judge it, and the summary with it: `issues` counts them and the state
    /// is `blocked` while one is left or a card is `unplayable`, else `manual_play` while a card is not `verified`.
    pub fn set_legality(&mut self, legality: Vec<Issue>) {
        self.summary.issues = u32::try_from(legality.len()).unwrap_or(u32::MAX);
        self.summary.state = if !legality.is_empty() || self.summary.counts.unplayable > 0 {
            DeckState::Blocked
        } else if self.summary.counts.verified < self.summary.counts.total {
            DeckState::ManualPlay
        } else {
            DeckState::Verified
        };
        self.legality = legality;
    }
}

/// Sums the copies of each `(zone, oracle id)`, in the case the ids are stored in.
fn merged(lines: &[DeckLine]) -> BTreeMap<(DeckZone, String), u32> {
    let mut merged = BTreeMap::new();
    for line in lines.iter().filter(|line| line.count > 0) {
        let copies = merged
            .entry((line.zone, line.oracle_id.to_ascii_lowercase()))
            .or_insert(0u32);
        *copies = copies.saturating_add(line.count);
    }
    merged
}

/// The name a report shows for a card; the oracle id when the card is not known.
fn name_of<'a>(facts: Option<&'a CardFacts>, oracle_id: &'a str) -> &'a str {
    facts.map_or(oracle_id, |facts| facts.printed.name.as_str())
}

/// Checks a deck: its legality in `format`, the support of each card and the hashes of the marker.
///
/// `cards` holds the facts of every card the database knows, keyed by oracle id in lower case; a
/// card of `lines` it lacks is reported as unplayable and as a legality issue. `library` and `caps`
/// decide each card's support (`Library::support`): the report depends on nothing else, so the same
/// deck gives the same report anywhere and a change of rules or engine shows in `coverage_hash`.
pub fn deck_report(
    format: &str,
    lines: &[DeckLine],
    cards: &BTreeMap<String, CardFacts>,
    library: &Library,
    caps: &Caps,
) -> DeckReport {
    let deck = merged(lines);
    let mut supports = BTreeMap::new();
    for (_, id) in deck.keys() {
        supports.entry(id.clone()).or_insert_with(|| {
            cards
                .get(id)
                .map(|facts| library.support(&facts.printed, caps))
        });
    }

    let coverage: Vec<(String, String)> = supports
        .iter()
        .map(|(id, record)| {
            let fingerprint = record
                .as_ref()
                .map_or_else(|| "unknown".to_owned(), |record| record.fingerprint.clone());
            (id.clone(), fingerprint)
        })
        .collect();

    let mut reported: Vec<ReportCard> = Vec::new();
    for zone in DeckZone::ALL {
        let mut in_zone: Vec<ReportCard> = deck
            .iter()
            .filter(|((z, _), _)| *z == zone)
            .map(|((_, id), count)| {
                let record = supports.get(id).and_then(Option::as_ref);
                let (support, reasons) = match record {
                    Some(record) => (
                        record.support,
                        record.clauses.iter().map(Reason::from).collect(),
                    ),
                    None => (
                        Support::Unplayable,
                        vec![Reason {
                            face: 0,
                            span: crate::card::Span { start: 0, end: 0 },
                            text: "the card is not in the card database".to_owned(),
                        }],
                    ),
                };
                ReportCard {
                    oracle_id: id.clone(),
                    name: name_of(cards.get(id), id).to_owned(),
                    zone,
                    count: *count,
                    support,
                    reasons,
                }
            })
            .collect();
        in_zone.sort_by_cached_key(|card| (card.name.to_lowercase(), card.oracle_id.clone()));
        reported.extend(in_zone);
    }

    let mut legality = legality::check(format, &deck, cards);
    legality.extend(companion::issues(format, &deck, cards, library, caps));
    let mut counts = Counts::default();
    let mut zones = Vec::new();
    for zone in DeckZone::ALL {
        let mut zone_counts = Counts::default();
        for card in reported.iter().filter(|card| card.zone == zone) {
            zone_counts.add(card.support, card.count);
            counts.add(card.support, card.count);
        }
        if zone_counts.total > 0 {
            zones.push(ZoneCounts {
                zone,
                counts: zone_counts,
            });
        }
    }
    let state = if !legality.is_empty() || counts.unplayable > 0 {
        DeckState::Blocked
    } else if counts.verified < counts.total {
        DeckState::ManualPlay
    } else {
        DeckState::Verified
    };
    DeckReport {
        deck_hash: hashes::deck_hash_of(&deck),
        coverage_hash: coverage_hash(&coverage),
        format: format.to_owned(),
        summary: Summary {
            format: format.to_owned(),
            state,
            counts,
            zones,
            issues: u32::try_from(legality.len()).unwrap_or(u32::MAX),
        },
        legality,
        cards: reported,
    }
}
