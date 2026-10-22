//! The precon decks as the server judges them: each one's deck check and whether a game takes it,
//! the unique code the API names it by, and the index of every precon that the list and the code
//! lookups share.
//!
//! One function judges a precon ([`Judge::judge`]), so the list, one precon and a seat that takes
//! it as its deck agree. The index is built from every stored precon and its cards and kept until
//! the precons, the cards, the printings or the compiled rules change; the rules files and the
//! engine's capabilities are fixed for the life of a [`CardSource`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use mtg_cards::deck::{
    CardFacts, Counts, DeckLine as CheckLine, DeckReport, DeckState, DeckZone, Issue, IssueCode,
    ZoneCounts, deck_report,
};
use mtg_cards::load::Library;
use mtg_cards::{Caps, CardDefinition};
use mtg_db::{
    DbError, DeckBoard, IndexVersion, PreconDeckDetail, PreconDeckId, PreconVersion, RulesVersion,
};
use mtg_protocol::rest::PreconListItem;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::cards::{CardSource, CatalogError, Rules, card_facts, printed_card};
use crate::deck_cards::Line;
use crate::deck_choice::{commander_ids, format_issues, library_ids};
use crate::recipe::FormatKind;

/// Longest code, in characters; a longer name is cut.
const MAX_CODE_CHARS: usize = 80;

/// Why the precons could not be had.
#[derive(Debug, Error)]
pub(crate) enum PreconError {
    /// The database failed.
    #[error("the precons could not be read: {0}")]
    Db(#[from] DbError),
    /// The rules files do not load.
    #[error("the precons could not be judged: {0}")]
    Catalog(#[from] CatalogError),
}

/// The board a precon entry sits on, as a zone of the deck check.
pub(crate) fn zone_of(board: DeckBoard) -> DeckZone {
    match board {
        DeckBoard::Commander => DeckZone::Commander,
        DeckBoard::Main => DeckZone::Main,
        DeckBoard::Sideboard => DeckZone::Sideboard,
    }
}

/// An entry of a precon that matches no stored card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Unresolved {
    pub(crate) zone: DeckZone,
    pub(crate) name: String,
    pub(crate) count: u32,
}

/// What the server makes of one precon.
#[derive(Debug)]
pub(crate) struct Judged {
    /// The format the deck check judged it in.
    pub(crate) format: &'static str,
    /// The deck check, entries without a card included.
    pub(crate) report: DeckReport,
    /// What keeps the deck out of a game: the legality issues and the rules of the game's format.
    pub(crate) issues: Vec<Issue>,
    /// Whether a game takes the deck: no issue and not blocked.
    pub(crate) playable: bool,
    /// The lines of the entries that have a card, in deck order.
    pub(crate) lines: Vec<Line>,
    /// The oracle id of every card of the main board in deck order: the library a game is given.
    pub(crate) library: Vec<String>,
    /// Their definitions, in the same order.
    pub(crate) cards: Vec<Arc<CardDefinition>>,
    /// The oracle id of every card of the commander board: the commanders a game starts with.
    pub(crate) commanders: Vec<String>,
    /// Their definitions, in the same order.
    pub(crate) leaders: Vec<Arc<CardDefinition>>,
}

impl Judged {
    /// The precon as the list shows it, under `code`.
    pub(crate) fn item(&self, deck: &PreconDeckDetail, code: &str) -> PreconListItem {
        let summary = &deck.summary;
        let commanders = deck
            .entries
            .iter()
            .filter(|entry| entry.board == DeckBoard::Commander)
            .map(|entry| {
                entry
                    .card
                    .as_ref()
                    .map_or_else(|| entry.card_name.clone(), |card| card.name.clone())
            })
            .collect();
        let card_count = deck
            .entries
            .iter()
            .filter(|entry| entry.board != DeckBoard::Sideboard)
            .map(|entry| entry.count)
            .fold(0, u32::saturating_add);
        let state = self.report.summary.state;
        PreconListItem {
            code: code.to_owned(),
            name: summary.name.clone(),
            set: summary.code.clone(),
            deck_type: summary.deck_type.clone(),
            release_date: summary.release_date.map(|date| date.to_string()),
            commanders,
            card_count,
            format: self.format.to_owned(),
            state,
            verified: state == DeckState::Verified,
            playable: self.playable,
            summary: self.report.summary.clone(),
        }
    }
}

/// Judges precons against one set of rules; remembers what it learned of each card, so judging every
/// precon assembles every card once.
pub(crate) struct Judge<'a> {
    library: &'a Library,
    caps: &'a Caps,
    facts: BTreeMap<String, CardFacts>,
    definitions: HashMap<String, Arc<CardDefinition>>,
}

impl<'a> Judge<'a> {
    /// A judge with the rules and capabilities a request reads cards with.
    pub(crate) fn new(rules: &'a Rules) -> Judge<'a> {
        Judge {
            library: rules.library(),
            caps: rules.caps(),
            facts: BTreeMap::new(),
            definitions: HashMap::new(),
        }
    }

    /// The deck check of a precon in its own format and the rules of a Normal game on top.
    ///
    /// A deck with a commander is judged as `commander`, any other as `freeform`: a precon is a
    /// product, not an entry in a tournament format.
    pub(crate) fn judge(&mut self, deck: &PreconDeckDetail) -> Judged {
        let mut lines = Vec::new();
        let mut unresolved = Vec::new();
        for entry in &deck.entries {
            let zone = zone_of(entry.board);
            match &entry.card {
                Some(card) => {
                    let id = card.oracle_id.to_string();
                    if !self.facts.contains_key(&id) {
                        let printed = printed_card(card);
                        let assembled = self.library.assemble(&printed, self.caps);
                        self.facts.insert(id.clone(), card_facts(card));
                        self.definitions
                            .insert(id.clone(), Arc::new(assembled.definition));
                    }
                    lines.push(Line {
                        zone,
                        oracle_id: id,
                        printing_id: entry.printing_id.as_ref().map(ToString::to_string),
                        count: entry.count,
                    });
                }
                None => unresolved.push(Unresolved {
                    zone,
                    name: entry.card_name.clone(),
                    count: entry.count,
                }),
            }
        }
        let commanded = deck
            .entries
            .iter()
            .any(|entry| entry.board == DeckBoard::Commander)
            || deck
                .summary
                .deck_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("Commander Deck"));
        let format = if commanded { "commander" } else { "freeform" };
        let check: Vec<CheckLine> = lines.iter().map(Line::to_check).collect();
        let mut report = deck_report(format, &check, &self.facts, self.library, self.caps);
        count_unresolved(&mut report, &unresolved);

        let library = library_ids(&lines);
        let cards: Vec<Arc<CardDefinition>> = library
            .iter()
            .filter_map(|id| self.definitions.get(id).cloned())
            .collect();
        let commanders = commander_ids(&lines);
        let leaders: Vec<Arc<CardDefinition>> = commanders
            .iter()
            .filter_map(|id| self.definitions.get(id).cloned())
            .collect();
        let kind = if commanded {
            FormatKind::Commander
        } else {
            FormatKind::Normal
        };
        // The engine's check is the second line: a deck the report already blocks needs no repeat.
        let mut issues = report.legality.clone();
        if issues.is_empty() {
            issues = format_issues(kind, &cards, &leaders);
        }
        let playable = issues.is_empty() && report.summary.state != DeckState::Blocked;
        Judged {
            format,
            report,
            issues,
            playable,
            lines,
            library,
            cards,
            commanders,
            leaders,
        }
    }
}

/// Adds the entries that match no card to a report: an `unknown_card` issue each, their copies as
/// `unplayable`, and a blocked deck.
fn count_unresolved(report: &mut DeckReport, unresolved: &[Unresolved]) {
    if unresolved.is_empty() {
        return;
    }
    for entry in unresolved {
        report.legality.push(Issue {
            code: IssueCode::UnknownCard,
            message: format!("{} is not in the card database", entry.name),
            zone: Some(entry.zone),
            oracle_id: None,
            name: Some(entry.name.clone()),
        });
        let summary = &mut report.summary;
        let add = |counts: &mut Counts| {
            counts.total = counts.total.saturating_add(entry.count);
            counts.unplayable = counts.unplayable.saturating_add(entry.count);
        };
        add(&mut summary.counts);
        if let Some(zone) = summary
            .zones
            .iter_mut()
            .find(|zone| zone.zone == entry.zone)
        {
            add(&mut zone.counts);
        } else {
            let mut counts = Counts::default();
            add(&mut counts);
            summary.zones.push(ZoneCounts {
                zone: entry.zone,
                counts,
            });
        }
    }
    report.summary.zones.sort_by_key(|zone| {
        DeckZone::ALL
            .iter()
            .position(|candidate| *candidate == zone.zone)
    });
    report.summary.issues = u32::try_from(report.legality.len()).unwrap_or(u32::MAX);
    report.summary.state = DeckState::Blocked;
}

/// The lower-case words of `text` joined by hyphens: ASCII letters and digits only, apostrophes
/// dropped, every other run of characters one hyphen.
fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in text.chars() {
        if c == '\'' || c == '\u{2019}' {
            continue;
        }
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    out
}

/// One code per deck, in the order given: the set code and the name's slug, cut at
/// [`MAX_CODE_CHARS`]; a code another deck already has ends in the deck's id.
pub(crate) fn codes(decks: &[(PreconDeckId, &str, &str)]) -> Vec<String> {
    let mut taken: HashSet<String> = HashSet::new();
    decks
        .iter()
        .map(|(id, set, name)| {
            let mut base = [slug(set), slug(name)]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("-");
            base.truncate(MAX_CODE_CHARS);
            let base = base.trim_end_matches('-').to_owned();
            let mut code = if base.is_empty() {
                "precon".to_owned()
            } else {
                base.clone()
            };
            let mut next = 0u32;
            while taken.contains(&code) {
                code = if next == 0 {
                    format!("{base}-{}", id.0)
                } else {
                    format!("{base}-{}-{next}", id.0)
                };
                next += 1;
            }
            taken.insert(code.clone());
            code
        })
        .collect()
}

/// One precon of the index.
#[derive(Debug)]
pub(crate) struct Indexed {
    /// The stored deck.
    pub(crate) id: PreconDeckId,
    /// The deck as the list shows it.
    pub(crate) item: PreconListItem,
}

/// Every stored precon with its judgment, in the order of the list.
#[derive(Debug, Default)]
pub(crate) struct PreconIndex {
    decks: Vec<Indexed>,
    by_code: HashMap<String, usize>,
}

impl PreconIndex {
    /// The precons, newest release first, then by name.
    pub(crate) fn items(&self) -> impl Iterator<Item = &PreconListItem> {
        self.decks.iter().map(|indexed| &indexed.item)
    }

    /// The precon with this code.
    pub(crate) fn find(&self, code: &str) -> Option<&Indexed> {
        self.by_code.get(code).and_then(|at| self.decks.get(*at))
    }

    fn build(rules: &Rules, details: &[PreconDeckDetail]) -> PreconIndex {
        let named: Vec<(PreconDeckId, &str, &str)> = details
            .iter()
            .map(|deck| {
                (
                    deck.summary.id,
                    deck.summary.code.as_str(),
                    deck.summary.name.as_str(),
                )
            })
            .collect();
        let codes = codes(&named);
        let mut judge = Judge::new(rules);
        let decks: Vec<Indexed> = details
            .iter()
            .zip(&codes)
            .map(|(deck, code)| Indexed {
                id: deck.summary.id,
                item: judge.judge(deck).item(deck, code),
            })
            .collect();
        let by_code = decks
            .iter()
            .enumerate()
            .map(|(at, indexed)| (indexed.item.code.clone(), at))
            .collect();
        PreconIndex { decks, by_code }
    }
}

/// What the stored precons, cards and compiled rules looked like when an index was built.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    cards: IndexVersion,
    precons: PreconVersion,
    rules: RulesVersion,
}

/// The index of the precons, built the first time it is needed and again when the precons, the
/// cards or the printings change; requests that arrive while it is built wait for that build.
#[derive(Debug, Default)]
pub(crate) struct PreconCache {
    built: Mutex<Option<(Stamp, Arc<PreconIndex>)>>,
}

impl PreconCache {
    /// The current index.
    pub(crate) async fn index(&self, source: &CardSource) -> Result<Arc<PreconIndex>, PreconError> {
        let database = source.database();
        let rules = source.rules().await?;
        let stamp = Stamp {
            cards: database.cards().index_version().await?,
            precons: database.precons().version().await?,
            rules: rules.version().clone(),
        };
        let mut built = self.built.lock().await;
        if let Some((held, index)) = built.as_ref()
            && *held == stamp
        {
            return Ok(index.clone());
        }
        let details = database.precons().all().await?;
        let index = Arc::new(PreconIndex::build(&rules, &details));
        *built = Some((stamp, index.clone()));
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a precon's code is its set and the slug of its name in lower-case ASCII words, cut to a bounded length and made unique by the deck's id, and entries that match no card block the deck and are counted as unplayable in the zone they sit in.
    use mtg_cards::deck::{DeckState, Summary};
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("Adaptive Enchantment", "adaptive-enchantment")]
    #[case("Atraxa, Praetors' Voice", "atraxa-praetors-voice")]
    #[case("They’re  Just   Like Us!", "theyre-just-like-us")]
    #[case("  --Élan Vital--  ", "lan-vital")]
    #[case("日本語", "")]
    #[case("", "")]
    fn a_name_becomes_lower_case_ascii_words(#[case] name: &str, #[case] expected: &str) {
        assert_eq!(slug(name), expected);
    }

    #[test]
    fn a_code_is_the_set_and_the_name_and_a_taken_one_ends_in_the_id() {
        let decks = [
            (PreconDeckId(1), "C18", "Adaptive Enchantment"),
            (PreconDeckId(2), "C18", "Adaptive: Enchantment"),
            (PreconDeckId(3), "C18", "Adaptive-Enchantment"),
            (PreconDeckId(4), "", "日本語"),
            (PreconDeckId(5), "SLD", "20 Ways to Win"),
        ];
        assert_eq!(
            codes(&decks),
            [
                "c18-adaptive-enchantment",
                "c18-adaptive-enchantment-2",
                "c18-adaptive-enchantment-3",
                "precon",
                "sld-20-ways-to-win"
            ]
        );
    }

    #[test]
    fn a_long_name_is_cut_without_a_trailing_hyphen_and_codes_stay_unique() {
        let name = "word ".repeat(40);
        let decks = [
            (PreconDeckId(1), "X", name.as_str()),
            (PreconDeckId(2), "X", name.as_str()),
        ];
        let made = codes(&decks);
        assert!(made.iter().all(|code| code.len() <= MAX_CODE_CHARS + 4));
        assert!(!made[0].ends_with('-'));
        assert_ne!(made[0], made[1]);
        assert!(
            made.iter()
                .flat_map(|code| code.chars())
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        );
    }

    #[test]
    fn an_id_clash_of_a_suffixed_code_goes_on_to_the_next_free_one() {
        let decks = [
            (PreconDeckId(1), "A", "B"),
            (PreconDeckId(2), "A", "B-2"),
            (PreconDeckId(2), "A", "B"),
        ];
        let made = codes(&decks);
        assert_eq!(made, ["a-b", "a-b-2", "a-b-2-1"]);
    }

    fn report() -> DeckReport {
        DeckReport {
            deck_hash: String::new(),
            coverage_hash: String::new(),
            format: "commander".into(),
            legality: Vec::new(),
            cards: Vec::new(),
            summary: Summary {
                format: "commander".into(),
                state: DeckState::Verified,
                counts: Counts {
                    total: 99,
                    verified: 99,
                    ..Counts::default()
                },
                zones: vec![ZoneCounts {
                    zone: DeckZone::Main,
                    counts: Counts {
                        total: 99,
                        verified: 99,
                        ..Counts::default()
                    },
                }],
                issues: 0,
            },
        }
    }

    #[test]
    fn entries_without_a_card_block_the_deck_and_count_as_unplayable_in_their_zone() {
        let mut report = report();
        count_unresolved(
            &mut report,
            &[
                Unresolved {
                    zone: DeckZone::Commander,
                    name: "Ghost Commander".into(),
                    count: 1,
                },
                Unresolved {
                    zone: DeckZone::Main,
                    name: "Ghost".into(),
                    count: 2,
                },
            ],
        );
        let summary = &report.summary;
        assert_eq!(summary.state, DeckState::Blocked);
        assert_eq!((summary.counts.total, summary.counts.unplayable), (102, 3));
        assert_eq!(summary.issues, 2);
        let zones: Vec<_> = summary
            .zones
            .iter()
            .map(|zone| (zone.zone, zone.counts.total, zone.counts.unplayable))
            .collect();
        assert_eq!(
            zones,
            [(DeckZone::Main, 101, 2), (DeckZone::Commander, 1, 1)]
        );
        assert_eq!(report.legality[0].code, IssueCode::UnknownCard);
        assert_eq!(report.legality[0].name.as_deref(), Some("Ghost Commander"));
    }

    #[test]
    fn a_report_without_unresolved_entries_is_left_alone() {
        let mut report = report();
        let before = report.clone();
        count_unresolved(&mut report, &[]);
        assert_eq!(report, before);
    }
}
