//! `compile`: generated rules for the stored cards, checked as rules files are and stored as `card_rules` rows next to the
//! hand-written files, which win for the same card.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mtg_cards::PrintedCard;
use mtg_cards::builtin;
use mtg_cards::card::ImplStatus as RulesStatus;
use mtg_cards::generated::{COMPILER_VERSION, Generation, generate, input_fingerprint};
use mtg_cards::load::{Library, LoadError};
use mtg_db::{Database, ImplStatus, NewRules, RuleStamp};
use mtg_import::ids::OracleId;

use crate::args::CompileArgs;
use crate::audit::stored_cards;
use crate::drafts::{self, DraftCounts};
use crate::error::CliError;
use crate::status::missing_table;

/// Rows written per transaction.
const BATCH: usize = 500;

/// How many cards with invalid rules the report lists.
const INVALID_SHOWN: usize = 20;

/// How many problems the report shows for one card.
const PROBLEMS_SHOWN: usize = 2;

/// A card whose compiled rules failed a check, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid {
    /// The card's name.
    pub card: String,
    /// The problems found, as `file: path: message`.
    pub problems: Vec<String>,
}

/// What a run did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompileReport {
    /// Which cards were compiled, in words.
    pub scope: String,
    /// How many cards were looked at.
    pub looked_at: usize,
    /// Rows written for cards that had none.
    pub written: usize,
    /// Rows written again because the fingerprint or the compiler version changed.
    pub replaced: usize,
    /// Rows left as they were.
    pub unchanged: usize,
    /// Rows removed because their card is no longer stored as a deck card.
    pub removed_gone: usize,
    /// Rows removed because a hand-written file covers the card.
    pub removed_hand_written: usize,
    /// Rows removed because the compiler no longer reads the card, or its rules no longer validate.
    pub removed_unread: usize,
    /// Cards left to their hand-written files.
    pub hand_written: usize,
    /// Of those, the cards whose file fails the checks against the stored card (an erratum, a changed keyword list).
    pub hand_written_invalid: Vec<Invalid>,
    /// Cards the compiler reads nothing of.
    pub nothing: usize,
    /// Cards whose compiled rules failed a check; no row is kept for them.
    pub invalid: Vec<Invalid>,
    /// Of the rows held for the cards looked at, those that cover the whole text.
    pub full: usize,
    /// Of the rows held for the cards looked at, those with clauses left to the table.
    pub partial: usize,
    /// What became of the drafts the build embeds.
    pub drafts: DraftCounts,
}

impl CompileReport {
    /// The report as text.
    pub fn render(&self) -> String {
        let removed = self.removed_gone + self.removed_hand_written + self.removed_unread;
        let mut text = format!(
            "compile: {}, {} cards\n  rows: {} written, {} replaced, {} unchanged, {removed} removed",
            self.scope, self.looked_at, self.written, self.replaced, self.unchanged
        );
        if removed > 0 {
            let _ = write!(
                text,
                " ({} card gone, {} hand-written, {} no longer read or valid)",
                self.removed_gone, self.removed_hand_written, self.removed_unread
            );
        }
        let _ = write!(
            text,
            "\n  by state: {} full, {} partial\n  not compiled: {} hand-written ({} invalid), {} nothing to read, {} compiled rules invalid",
            self.full,
            self.partial,
            self.hand_written,
            self.hand_written_invalid.len(),
            self.nothing,
            self.invalid.len()
        );
        if !self.hand_written_invalid.is_empty() {
            text.push_str("\n  hand-written files that fail the stored cards:");
            list_invalid(&mut text, &self.hand_written_invalid);
        }
        list_invalid(&mut text, &self.invalid);
        let _ = write!(text, "\n{}", self.drafts.render());
        text
    }
}

/// The first cards of `invalid` with their problems, and how many more there are.
fn list_invalid(text: &mut String, invalid: &[Invalid]) {
    for card in invalid.iter().take(INVALID_SHOWN) {
        let _ = write!(text, "\n    {}:", card.card);
        for problem in &card.problems {
            let _ = write!(text, "\n      {problem}");
        }
    }
    if invalid.len() > INVALID_SHOWN {
        let _ = write!(text, "\n    and {} more", invalid.len() - INVALID_SHOWN);
    }
}

/// What a run changes in the table.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    /// Rows to write.
    pub put: Vec<NewRules>,
    /// Rows to remove.
    pub remove: Vec<OracleId>,
    /// The report, as it will read once the plan is done.
    pub report: CompileReport,
}

/// The lowercase names of the cards and their faces: what a rule may name.
pub(crate) fn names_of(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| {
            std::iter::once(&card.name).chain(card.faces.iter().map(|face| &face.name))
        })
        .map(|name| name.to_lowercase())
        .collect()
}

fn db_status(status: RulesStatus) -> Option<ImplStatus> {
    match status {
        RulesStatus::Full => Some(ImplStatus::Full),
        RulesStatus::Partial => Some(ImplStatus::Partial),
        RulesStatus::Manual | RulesStatus::Unplayable => None,
    }
}

/// The card with the first of its problems, as the report lists them.
fn invalid(printed: &PrintedCard, problems: &[LoadError]) -> Invalid {
    Invalid {
        card: printed.name.clone(),
        problems: problems
            .iter()
            .take(PROBLEMS_SHOWN)
            .map(ToString::to_string)
            .collect(),
    }
}

/// The status and the JSON of the rules the compiler makes of a card that pass the checks; a card it reads nothing of or
/// whose rules fail a check is counted in the report instead.
fn compiled(
    printed: &PrintedCard,
    hand_written: &Library,
    names: &BTreeSet<String>,
    report: &mut CompileReport,
) -> Option<(ImplStatus, serde_json::Value)> {
    match generate(printed, hand_written, names) {
        Generation::Nothing => {
            report.nothing += 1;
            None
        }
        Generation::Invalid(problems) => {
            report.invalid.push(invalid(printed, &problems));
            None
        }
        Generation::Rules(card) => {
            let rules = db_status(card.impl_status).zip(serde_json::to_value(&*card).ok());
            if rules.is_none() {
                report.nothing += 1;
            }
            rules
        }
    }
}

/// What to do about the stored `cards` (every deck card, so that rows of cards that are gone are found) and the rows
/// `stamps` hold, for the cards in `selected` (all of them when `None`).
pub fn plan(
    cards: &[PrintedCard],
    selected: Option<&BTreeSet<String>>,
    stamps: &[RuleStamp],
    hand_written: &Library,
    scope: &str,
) -> Plan {
    let names = names_of(cards);
    let stored: BTreeSet<&str> = cards.iter().map(|card| card.oracle_id.as_str()).collect();
    let rows: BTreeMap<&str, &RuleStamp> = stamps
        .iter()
        .map(|stamp| (stamp.oracle_id.as_str(), stamp))
        .collect();
    let mut plan = Plan {
        report: CompileReport {
            scope: scope.to_owned(),
            ..CompileReport::default()
        },
        ..Plan::default()
    };
    for stamp in stamps {
        let id = stamp.oracle_id.as_str();
        if !stored.contains(id) {
            plan.report.removed_gone += 1;
            plan.remove.push(stamp.oracle_id.clone());
        } else if hand_written.card(id).is_some() {
            plan.report.removed_hand_written += 1;
            plan.remove.push(stamp.oracle_id.clone());
        }
    }
    let Plan {
        put,
        remove,
        report,
    } = &mut plan;
    for printed in cards
        .iter()
        .filter(|card| selected.is_none_or(|ids| ids.contains(&card.oracle_id)))
    {
        report.looked_at += 1;
        if let Some(file) = hand_written.card(&printed.oracle_id) {
            report.hand_written += 1;
            let problems = hand_written.check_generated(file, printed, &names);
            if !problems.is_empty() {
                report
                    .hand_written_invalid
                    .push(invalid(printed, &problems));
            }
            continue;
        }
        let row = rows.get(printed.oracle_id.as_str());
        let fingerprint = input_fingerprint(printed);
        if let Some(row) = row
            && row.fingerprint == fingerprint
            && row.compiler_version == COMPILER_VERSION
        {
            report.unchanged += 1;
            match row.status {
                ImplStatus::Full => report.full += 1,
                ImplStatus::Partial => report.partial += 1,
                ImplStatus::Manual => {}
            }
            continue;
        }
        match compiled(printed, hand_written, &names, report) {
            Some((status, rules)) => {
                if status == ImplStatus::Partial {
                    report.partial += 1;
                } else {
                    report.full += 1;
                }
                if row.is_some() {
                    report.replaced += 1;
                } else {
                    report.written += 1;
                }
                put.push(NewRules {
                    oracle_id: OracleId(printed.oracle_id.clone()),
                    rules,
                    status,
                    fingerprint,
                    compiler_version: COMPILER_VERSION.to_owned(),
                });
            }
            None if row.is_some() => {
                report.removed_unread += 1;
                remove.push(OracleId(printed.oracle_id.clone()));
            }
            None => {}
        }
    }
    plan
}

/// The oracle ids of the cards of every stored precon.
async fn precon_cards(db: &Database) -> Result<BTreeSet<String>, CliError> {
    let decks = db.precons().all().await?;
    Ok(decks
        .iter()
        .flat_map(|deck| &deck.entries)
        .filter_map(|entry| entry.card.as_ref())
        .map(|card| card.oracle_id.as_str().to_owned())
        .collect())
}

/// Compiles the cards the arguments select, writes the rules that validate and removes the rows that no longer belong.
pub async fn compile(db: &Database, args: &CompileArgs) -> Result<CompileReport, CliError> {
    let hand_written = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let cards = stored_cards(db).await?;
    if cards.is_empty() {
        return Err(CliError::Config(
            "no cards are stored; `mtg-cli fetch scryfall` and `mtg-cli ingest` load them".into(),
        ));
    }
    let repo = db.card_rules();
    let stamps = repo.stamps().await.map_err(|error| {
        if missing_table(&error) {
            CliError::Config("the schema is not applied yet; `mtg-cli ingest` applies it".into())
        } else {
            CliError::Db(error)
        }
    })?;
    let (selected, scope) = match (&args.card, args.all) {
        (Some(wanted), _) => {
            let lower = wanted.to_lowercase();
            let ids: BTreeSet<String> = cards
                .iter()
                .filter(|card| {
                    card.name.to_lowercase() == lower
                        || card
                            .faces
                            .iter()
                            .any(|face| face.name.to_lowercase() == lower)
                })
                .map(|card| card.oracle_id.clone())
                .collect();
            if ids.is_empty() {
                return Err(CliError::Config(format!(
                    "no stored card is named `{wanted}`"
                )));
            }
            (Some(ids), format!("the card {wanted}"))
        }
        (None, true) => (None, "every stored card".to_owned()),
        (None, false) => {
            let ids = precon_cards(db).await?;
            if ids.is_empty() {
                return Err(CliError::Config(
                    "no precons are stored; `mtg-cli fetch decks` and `mtg-cli ingest` load them, or compile every card with `--all`".into(),
                ));
            }
            (Some(ids), "the cards of the stored precons".to_owned())
        }
    };
    let plan = plan(&cards, selected.as_ref(), &stamps, hand_written, &scope);
    for batch in plan.remove.chunks(BATCH) {
        repo.remove(batch).await?;
    }
    for batch in plan.put.chunks(BATCH) {
        repo.put(batch).await?;
    }
    let mut report = plan.report;
    report.drafts = drafts::store(db, hand_written, &mtg_engine::caps(), &cards).await?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    //! Goal: a plan writes a card the compiler reads, skips one a hand-written file covers, keeps a row whose fingerprint and compiler version are current, writes again one that is not, and removes the rows of cards that are gone, that a file covers or that the compiler no longer reads; the report lists a bounded number of invalid cards with their reasons.
    use mtg_cards::PrintedFace;
    use rstest::rstest;

    use super::*;

    fn card(id: u64, name: &str, text: &str) -> PrintedCard {
        PrintedCard {
            layout: String::new(),
            oracle_id: format!("00000000-0000-0000-0000-{id:012}"),
            name: name.into(),
            keywords: Vec::new(),
            faces: vec![PrintedFace {
                name: name.into(),
                mana_cost: "{1}".into(),
                type_line: "Instant".into(),
                oracle_text: text.into(),
                ..PrintedFace::default()
            }],
        }
    }

    fn row(printed: &PrintedCard, version: &str) -> RuleStamp {
        RuleStamp {
            oracle_id: OracleId(printed.oracle_id.clone()),
            status: ImplStatus::Full,
            fingerprint: input_fingerprint(printed),
            compiler_version: version.into(),
        }
    }

    const DRAW: &str = "Draw a card.";

    fn run(cards: &[PrintedCard], stamps: &[RuleStamp]) -> Plan {
        plan(cards, None, stamps, &Library::default(), "tests")
    }

    #[test]
    fn a_card_the_compiler_reads_is_written() {
        let cards = [card(1, "Opt", DRAW)];

        let plan = run(&cards, &[]);

        assert_eq!(plan.put.len(), 1);
        assert_eq!(plan.put[0].status, ImplStatus::Full);
        assert_eq!(plan.put[0].compiler_version, COMPILER_VERSION);
        assert_eq!(plan.put[0].fingerprint, input_fingerprint(&cards[0]));
        assert_eq!((plan.report.written, plan.report.full), (1, 1));
        assert!(plan.remove.is_empty(), "{:?}", plan.remove);
    }

    #[test]
    fn a_current_row_is_left_alone_and_any_other_is_written_again() {
        let (current, other_version, other_text) = (
            card(1, "Opt", DRAW),
            card(2, "Brainstorm", DRAW),
            card(3, "Preordain", DRAW),
        );
        let stamps = [
            row(&current, COMPILER_VERSION),
            row(&other_version, "old"),
            RuleStamp {
                fingerprint: "0".repeat(32),
                ..row(&other_text, COMPILER_VERSION)
            },
        ];

        let plan = run(&[current, other_version, other_text], &stamps);

        assert_eq!(
            (
                plan.report.unchanged,
                plan.report.replaced,
                plan.report.written
            ),
            (1, 2, 0)
        );
        assert_eq!(plan.put.len(), 2);
        assert!(plan.remove.is_empty(), "{:?}", plan.remove);
    }

    #[test]
    fn rows_that_no_longer_belong_are_removed() {
        let (nothing, kept, gone) = (
            card(1, "Plains", ""),
            card(2, "Opt", DRAW),
            card(3, "Gone", DRAW),
        );
        let stamps = [
            row(&nothing, "old"),
            row(&kept, COMPILER_VERSION),
            row(&gone, COMPILER_VERSION),
        ];

        let plan = run(&[nothing.clone(), kept], &stamps);

        assert_eq!(
            plan.remove,
            [
                OracleId(gone.oracle_id.clone()),
                OracleId(nothing.oracle_id.clone())
            ]
        );
        assert_eq!(
            (
                plan.report.removed_gone,
                plan.report.removed_unread,
                plan.report.unchanged
            ),
            (1, 1, 1)
        );
    }

    #[test]
    fn a_selection_compiles_its_cards_and_prunes_only_what_cannot_belong() {
        let (one, two, gone) = (
            card(1, "Opt", DRAW),
            card(2, "Brainstorm", DRAW),
            card(3, "Gone", DRAW),
        );
        let stamps = [row(&two, "old"), row(&gone, COMPILER_VERSION)];
        let selected = BTreeSet::from([one.oracle_id.clone()]);

        let plan = plan(
            &[one, two.clone()],
            Some(&selected),
            &stamps,
            &Library::default(),
            "one",
        );

        assert_eq!((plan.report.looked_at, plan.report.written), (1, 1));
        assert_eq!(plan.remove, [OracleId(gone.oracle_id)]);
        assert!(
            plan.put
                .iter()
                .all(|row| row.oracle_id.as_str() != two.oracle_id)
        );
    }

    fn bolt() -> PrintedCard {
        card(
            1,
            "Lightning Bolt",
            "Lightning Bolt deals 3 damage to any target.",
        )
    }

    /// The library of the one hand-written file, a `Manual` Lightning Bolt.
    fn bolt_file(bolt: &PrintedCard) -> Library {
        Library::from_sources(&[mtg_cards::load::Source {
            path: "cards/lightning_bolt.json".into(),
            text: format!(
                r#"{{"oracle_id":"{}","name":"Lightning Bolt","impl_status":"Manual","faces":[{{"name":"Lightning Bolt","oracle_fingerprint":"{}","abilities":[]}}]}}"#,
                bolt.oracle_id,
                input_fingerprint(bolt)
            ),
        }])
        .unwrap()
    }

    #[rstest]
    #[case::the_card_as_written(|_: &mut PrintedCard| {}, None)]
    #[case::a_renamed_card(|card: &mut PrintedCard| card.name = "Lightning Strike".into(), Some("imported name"))]
    #[case::a_second_face(
        |card: &mut PrintedCard| card.faces.push(card.faces[0].clone()),
        Some("face(s)")
    )]
    fn a_hand_written_file_that_fails_the_stored_card_is_named(
        #[case] change: fn(&mut PrintedCard),
        #[case] problem: Option<&str>,
    ) {
        let (written, mut stored) = (bolt(), bolt());
        let hand_written = bolt_file(&written);
        change(&mut stored);

        let report = plan(&[stored], None, &[], &hand_written, "tests").report;

        assert_eq!(report.hand_written, 1);
        assert!(report.invalid.is_empty(), "{:?}", report.invalid);
        let text = report.render();
        match problem {
            None => {
                assert!(report.hand_written_invalid.is_empty(), "{text}");
                assert!(text.contains("1 hand-written (0 invalid)"), "{text}");
                assert!(!text.contains("fail the stored cards"), "{text}");
            }
            Some(problem) => {
                assert_eq!(report.hand_written_invalid.len(), 1, "{text}");
                assert!(text.contains("1 hand-written (1 invalid)"), "{text}");
                assert!(
                    text.contains("hand-written files that fail the stored cards:\n    "),
                    "{text}"
                );
                assert!(text.contains(problem), "{text}");
            }
        }
    }

    #[test]
    fn a_hand_written_card_is_skipped_and_its_row_removed() {
        let bolt = bolt();
        let hand_written = bolt_file(&bolt);

        let plan = plan(
            std::slice::from_ref(&bolt),
            None,
            &[row(&bolt, "old")],
            &hand_written,
            "tests",
        );

        assert!(plan.put.is_empty(), "{:?}", plan.put);
        assert_eq!(plan.remove, [OracleId(bolt.oracle_id)]);
        assert_eq!(
            (plan.report.hand_written, plan.report.removed_hand_written),
            (1, 1)
        );
    }

    #[test]
    fn the_report_lists_a_bounded_number_of_invalid_cards_with_their_reasons() {
        let report = CompileReport {
            scope: "tests".into(),
            looked_at: 30,
            written: 3,
            removed_gone: 1,
            removed_unread: 2,
            invalid: (0..22)
                .map(|n| Invalid {
                    card: format!("Card {n}"),
                    problems: vec![format!("cards/card_{n}.json: faces: why {n}")],
                })
                .collect(),
            ..CompileReport::default()
        };

        let text = report.render();

        assert!(
            text.contains("3 written, 0 replaced, 0 unchanged, 3 removed (1 card gone, 0 hand-written, 2 no longer read or valid)"),
            "{text}"
        );
        assert!(
            text.contains(
                "22 compiled rules invalid\n    Card 0:\n      cards/card_0.json: faces: why 0"
            ),
            "{text}"
        );
        assert!(
            text.contains("Card 19:") && !text.contains("Card 20:"),
            "{text}"
        );
        assert!(
            text.contains("and 2 more\n  drafts: 0 in the build"),
            "{text}"
        );
    }
}
