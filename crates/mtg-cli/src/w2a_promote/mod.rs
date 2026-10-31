//! `promote`: the proposals the checker accepts become corpus cards, a rules file in `cards/` and the card's Scryfall
//! record from the cache as its fixture (`docs/cards-dsl.md`, Model-assisted reading).

pub mod draft;
pub mod fixture;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mtg_cards::compile::compile;
use mtg_cards::load::{Library, canonical, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::{Caps, PrintedCard, Support, support};
use mtg_cards::{builtin, card::Card};
use mtg_db::Database;

use crate::args::PromoteArgs;
use crate::audit::stored_cards;
use crate::cache::Cache;
use crate::compile::names_of;
use crate::drafts::{self, SmokeKind};
use crate::error::CliError;
use fixture::{w2a_oracle_cards, w2a_records};

/// Where `promote` writes when `--out` is not given: the `mtg-cards` crate, seen from the repository root.
pub const W2A_DEFAULT_OUT: &str = "crates/mtg-cards";

/// Why a proposal is not promoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W2aSkip {
    /// No stored card has the file's name as the slug of its name.
    NoCard,
    /// The card already has a corpus rules file or fixture.
    CorpusFile,
    /// The compiler already makes the card `Verified`.
    Compiled,
    /// The checker does not accept the proposal.
    Rejected(Verdict),
    /// Accepted, but with a state that is not promoted (`Partial` is, with `--partial`).
    State(Support),
    /// The rules name a card without a corpus file (a partner), so the corpus would not load.
    NamesOutside,
    /// An earlier file of the run promotes the same card.
    Repeated,
    /// The cached Oracle Cards file has no record of the card.
    NoRecord,
    /// An `Unchecked` proposal that is not a whole card: it has a `Manual` ability or a status other than `full`.
    NotWhole,
    /// The engine would not run the draft as a `Read` card (the loader refuses it or a primitive is missing).
    NotRead,
    /// The draft fails the smoke gate.
    Smoke(SmokeKind),
}

impl W2aSkip {
    /// The reason in a few words.
    pub fn reason(self) -> String {
        match self {
            W2aSkip::NoCard => "no stored card is named after the file".into(),
            W2aSkip::CorpusFile => "already a corpus card".into(),
            W2aSkip::Compiled => "the compiler makes it Verified".into(),
            W2aSkip::Rejected(verdict) => verdict.word().to_owned(),
            W2aSkip::State(state) => format!("accepted {state:?}"),
            W2aSkip::NamesOutside => "names a card outside the corpus".into(),
            W2aSkip::Repeated => "an earlier file promotes it".into(),
            W2aSkip::NoRecord => "no record in the cached Oracle Cards file".into(),
            W2aSkip::NotWhole => "unchecked, and not a whole card".into(),
            W2aSkip::NotRead => "the engine does not run it as a Read card".into(),
            W2aSkip::Smoke(kind) => format!("fails the smoke gate: {}", kind.word()),
        }
    }
}

/// A proposal to promote: the card and its rules as the checker completes them.
#[derive(Debug, Clone, PartialEq)]
pub struct W2aPromotion {
    /// The file name of the rules file and the fixture, without `.json`.
    pub slug: String,
    /// The card's oracle id.
    pub oracle_id: String,
    /// The support state of the rules.
    pub state: Support,
    /// The completed rules.
    pub rules: Card,
    /// Whether it becomes a draft for players to read (an `Unchecked` proposal) and not a corpus card.
    pub draft: bool,
}

/// What becomes of one proposal file.
#[derive(Debug, Clone, PartialEq)]
pub struct W2aDecision {
    /// The proposal file.
    pub file: PathBuf,
    /// The card's name, or the file's name when no card is found.
    pub card: String,
    /// The promotion, or why there is none.
    pub outcome: Result<W2aPromotion, W2aSkip>,
    /// More about a skip, such as the game that failed the smoke gate.
    pub detail: Option<String>,
}

/// What the decisions are made against.
#[derive(Debug, Clone, Copy)]
pub struct W2aBasis<'a> {
    /// The stored cards.
    pub cards: &'a [PrintedCard],
    /// The hand-written rules files.
    pub library: &'a Library,
    /// The lowercase names a rule may name.
    pub names: &'a BTreeSet<String>,
    /// The lowercase names of the corpus cards and their faces: the names a corpus card may name.
    pub corpus: &'a BTreeSet<String>,
    /// What the engine runs.
    pub caps: &'a Caps,
    /// The `mtg-cards` crate directory the cards go to.
    pub out: &'a Path,
    /// Whether accepted `Partial` proposals are promoted too.
    pub partial: bool,
    /// Whether `Unchecked` proposals that are whole cards become drafts.
    pub draft: bool,
}

impl W2aBasis<'_> {
    fn has_corpus_file(&self, printed: &PrintedCard, slug: &str) -> bool {
        let file = format!("{slug}.json");
        self.library.card(&printed.oracle_id).is_some()
            || self.out.join("cards").join(&file).is_file()
            || self.out.join(W2A_FIXTURES).join(&file).is_file()
    }

    fn decide_one(&self, printed: &PrintedCard, text: &str) -> Result<W2aPromotion, W2aSkip> {
        let slug = slug(&printed.name);
        if self.has_corpus_file(printed, &slug) {
            return Err(W2aSkip::CorpusFile);
        }
        let compiled = compile(printed).card;
        if support(compiled.as_ref(), printed, self.caps).support == Support::Verified {
            return Err(W2aSkip::Compiled);
        }
        let report = check(text, printed, self.library, self.names, self.caps);
        let verdict = report.verdict();
        if self.draft && verdict == Verdict::Unchecked {
            return draft::decide(printed, slug, report);
        }
        let (Verdict::Accepted, Some(record), Some(rules)) =
            (verdict, report.support, report.rules)
        else {
            return Err(W2aSkip::Rejected(verdict));
        };
        let state = record.support;
        if state != Support::Verified && !(self.partial && state == Support::Partial) {
            return Err(W2aSkip::State(state));
        }
        if w2a_names_outside(self.library, &rules, printed, self.corpus) {
            return Err(W2aSkip::NamesOutside);
        }
        Ok(W2aPromotion {
            slug,
            oracle_id: printed.oracle_id.clone(),
            state,
            rules,
            draft: false,
        })
    }
}

/// The lowercase names of the cards of a library and of their faces.
pub fn w2a_corpus_names(library: &Library) -> BTreeSet<String> {
    library
        .cards()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

/// Whether the rules fail their checks once only the corpus names count: they name a card without a corpus file.
pub fn w2a_names_outside(
    library: &Library,
    rules: &Card,
    printed: &PrintedCard,
    corpus: &BTreeSet<String>,
) -> bool {
    !library.check_generated(rules, printed, corpus).is_empty()
}

/// The fixture directory below the `mtg-cards` crate.
const W2A_FIXTURES: &str = "tests/fixtures/corpus";

/// Decides each proposal file (its path and text) in order; a card is promoted by its first file only.
pub fn w2a_decide(basis: &W2aBasis, files: &[(PathBuf, String)]) -> Vec<W2aDecision> {
    let mut by_slug: BTreeMap<String, &PrintedCard> = BTreeMap::new();
    for card in basis.cards {
        by_slug.entry(slug(&card.name)).or_insert(card);
    }
    let mut promoted = BTreeSet::new();
    let mut decisions = Vec::new();
    for (file, text) in files {
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let Some(printed) = by_slug.get(stem) else {
            decisions.push(W2aDecision {
                file: file.clone(),
                card: stem.to_owned(),
                outcome: Err(W2aSkip::NoCard),
                detail: None,
            });
            continue;
        };
        let outcome = if promoted.contains(&printed.oracle_id) {
            Err(W2aSkip::Repeated)
        } else {
            basis.decide_one(printed, text)
        };
        // Drafts are sorted out after the smoke gate: the first one that passes wins.
        if matches!(&outcome, Ok(promotion) if !promotion.draft) {
            promoted.insert(printed.oracle_id.clone());
        }
        decisions.push(W2aDecision {
            file: file.clone(),
            card: printed.name.clone(),
            outcome,
            detail: None,
        });
    }
    decisions
}

/// Turns each promotion whose card has no record into a skip, and returns the fixture texts by oracle id.
pub fn w2a_attach_records(
    decisions: &mut [W2aDecision],
    oracle_cards: &Path,
) -> Result<BTreeMap<String, String>, CliError> {
    let wanted: BTreeSet<String> = decisions
        .iter()
        .filter_map(|decision| decision.outcome.as_ref().ok())
        .map(|promotion| promotion.oracle_id.clone())
        .collect();
    let records = w2a_records(oracle_cards, &wanted)?;
    for decision in decisions {
        if let Ok(promotion) = &decision.outcome
            && !records.contains_key(&promotion.oracle_id)
        {
            decision.outcome = Err(W2aSkip::NoRecord);
        }
    }
    Ok(records)
}

/// What a run wrote.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct W2aWritten {
    /// The rules files written, corpus cards and drafts.
    pub files: Vec<PathBuf>,
    /// The slugs of the drafts removed because their card became a corpus card.
    pub superseded: Vec<String>,
}

/// Writes the rules file and the fixture of every promotion, a corpus card below `out` and a draft below `drafts`; a
/// corpus card removes the draft of the same card.
pub fn w2a_write(
    out: &Path,
    drafts: &Path,
    decisions: &[W2aDecision],
    records: &BTreeMap<String, String>,
) -> Result<W2aWritten, CliError> {
    let mut written = W2aWritten::default();
    for promotion in decisions.iter().filter_map(|d| d.outcome.as_ref().ok()) {
        let Some(record) = records.get(&promotion.oracle_id) else {
            continue;
        };
        if promotion.draft {
            written.files.push(draft::write(drafts, promotion, record)?);
            continue;
        }
        let file = format!("{}.json", promotion.slug);
        let rules = out.join("cards").join(&file);
        let fixture = out.join(W2A_FIXTURES).join(&file);
        std::fs::write(&rules, canonical(&promotion.rules))
            .map_err(|source| CliError::io(&rules, source))?;
        std::fs::write(&fixture, record).map_err(|source| CliError::io(&fixture, source))?;
        written.files.push(rules);
        if draft::remove_superseded(drafts, &promotion.slug)? {
            written.superseded.push(promotion.slug.clone());
        }
    }
    Ok(written)
}

/// The decisions of a run and the rules files it wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct W2aPromoted {
    /// One decision for each proposal file.
    pub decisions: Vec<W2aDecision>,
    /// The rules files written; empty in a dry run.
    pub written: Vec<PathBuf>,
    /// The slugs of the drafts removed because their card became a corpus card.
    pub superseded: Vec<String>,
    /// Whether nothing was written.
    pub dry_run: bool,
    /// Whether `Unchecked` proposals were considered as drafts.
    pub drafting: bool,
}

impl W2aPromoted {
    /// One line for each file, then the counts.
    pub fn render(&self) -> String {
        let (verb, draft_verb) = if self.dry_run {
            ("would promote", "would draft")
        } else {
            ("promoted", "drafted")
        };
        let mut text = String::new();
        let mut skips: BTreeMap<String, usize> = BTreeMap::new();
        let (mut count, mut drafts) = (0, 0);
        for decision in &self.decisions {
            match &decision.outcome {
                Ok(promotion) if promotion.draft => {
                    drafts += 1;
                    let _ = writeln!(
                        text,
                        "{draft_verb} {}: drafts/{}.json",
                        decision.card, promotion.slug
                    );
                }
                Ok(promotion) => {
                    count += 1;
                    let state = promotion.state;
                    let _ = writeln!(
                        text,
                        "{verb} {} ({state:?}): cards/{}.json",
                        decision.card, promotion.slug
                    );
                }
                Err(skip) => {
                    *skips.entry(skip.reason()).or_insert(0) += 1;
                    let _ = write!(
                        text,
                        "skipped {} ({}): {}",
                        decision.card,
                        decision.file.display(),
                        skip.reason()
                    );
                    if let Some(detail) = &decision.detail {
                        let _ = write!(text, " ({detail})");
                    }
                    text.push('\n');
                }
            }
        }
        for slug in &self.superseded {
            let _ = writeln!(text, "removed the draft of {slug}: it is a corpus card now");
        }
        let tally: Vec<String> = skips
            .iter()
            .map(|(reason, n)| format!("{n} {reason}"))
            .collect();
        let _ = write!(text, "{count} {verb}");
        if self.drafting || drafts > 0 {
            let _ = write!(text, ", {drafts} {draft_verb}");
        }
        let _ = write!(text, ", {} skipped", self.decisions.len() - count - drafts);
        if !tally.is_empty() {
            let _ = write!(text, ": {}", tally.join(", "));
        }
        text
    }
}

/// Reads the files, decides each one against the stored cards and, unless it is a dry run, writes the promotions.
pub async fn w2a_promote(
    db: &Database,
    cache: &Cache,
    args: &PromoteArgs,
) -> Result<W2aPromoted, CliError> {
    let library = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    w2a_promote_against(db, cache, args, library).await
}

/// `promote` against these hand-written files instead of the embedded ones.
pub async fn w2a_promote_against(
    db: &Database,
    cache: &Cache,
    args: &PromoteArgs,
    library: &Library,
) -> Result<W2aPromoted, CliError> {
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(W2A_DEFAULT_OUT));
    if !out.join("cards").is_dir() || !out.join(W2A_FIXTURES).is_dir() {
        return Err(CliError::Config(format!(
            "{}: not the mtg-cards crate (it has cards/ and {W2A_FIXTURES}/); name it with --out",
            out.display()
        )));
    }
    let mut files = Vec::new();
    for file in &args.files {
        let text = tokio::fs::read_to_string(file)
            .await
            .map_err(|source| CliError::io(file, source))?;
        files.push((file.clone(), text));
    }
    let cards = stored_cards(db).await?;
    let names = names_of(&cards);
    let caps = mtg_engine::caps();
    let corpus = w2a_corpus_names(library);
    let basis = W2aBasis {
        cards: &cards,
        library,
        names: &names,
        corpus: &corpus,
        caps: &caps,
        out: &out,
        partial: args.partial,
        draft: args.draft,
    };
    let mut decisions = w2a_decide(&basis, &files);
    let records = w2a_attach_records(&mut decisions, &w2a_oracle_cards(cache)?)?;
    if args.draft {
        draft::gate(&basis, &mut decisions, !args.no_smoke);
    }
    let drafts = args
        .drafts
        .clone()
        .unwrap_or_else(|| PathBuf::from(drafts::DEFAULT_DIR));
    let written = if args.dry_run {
        W2aWritten::default()
    } else {
        w2a_write(&out, &drafts, &decisions, &records)?
    };
    Ok(W2aPromoted {
        decisions,
        written: written.files,
        superseded: written.superseded,
        dry_run: args.dry_run,
        drafting: args.draft,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a card whose rules name a partner is promoted only when the partner is a corpus card, and rules that name
    //! no card always pass; the corpus names are the lowercase card and face names.
    #![allow(clippy::unwrap_used)]

    use mtg_cards::support::PrintedFace;
    use rstest::rstest;

    use super::*;

    fn printed(text: &str) -> PrintedCard {
        PrintedCard {
            oracle_id: "00000000-0000-4000-8000-00000000a1fa".into(),
            name: "Alpha".into(),
            keywords: Vec::new(),
            layout: "normal".into(),
            faces: vec![PrintedFace {
                name: "Alpha".into(),
                mana_cost: "{1}{W}".into(),
                type_line: "Legendary Creature — Human".into(),
                oracle_text: text.into(),
                power: Some("2".into()),
                toughness: Some("2".into()),
                ..PrintedFace::default()
            }],
        }
    }

    const PARTNER: &str = r#"{"Keyword":{"PartnerWith":"Beta Prime"}}"#;

    #[rstest]
    #[case("Partner with Beta Prime", PARTNER, &[], true)]
    #[case("Partner with Beta Prime", PARTNER, &["gamma"], true)]
    #[case("Partner with Beta Prime", PARTNER, &["beta prime"], false)]
    #[case("Flying", r#"{"Keyword":"Flying"}"#, &[], false)]
    fn a_partner_must_be_a_corpus_card(
        #[case] text: &str,
        #[case] ability: &str,
        #[case] corpus: &[&str],
        #[case] outside: bool,
    ) {
        let card = printed(text);
        let library = Library::default();
        let names: BTreeSet<String> = ["alpha", "beta prime"].map(String::from).into();
        let json = format!(r#"{{"faces":[{{"abilities":[{ability}]}}]}}"#);
        let report = check(&json, &card, &library, &names, &mtg_engine::caps());
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        let corpus: BTreeSet<String> = corpus.iter().map(|name| (*name).to_owned()).collect();
        let rules = report.rules.unwrap();
        assert_eq!(w2a_names_outside(&library, &rules, &card, &corpus), outside);
    }

    #[test]
    fn the_corpus_names_are_the_lowercase_card_and_face_names() {
        let names = w2a_corpus_names(builtin::library().unwrap());
        assert!(
            names.contains("lightning bolt")
                && names.contains("fire")
                && names.contains("fire // ice")
        );
        assert!(names.iter().all(|name| *name == name.to_lowercase()));
    }
}
