//! `semantic-audit`: cards that play automatically whose rules differ in structure from their Oracle text (an intervening
//! "if" with no condition, "for each" with no count, a dropped "may", "another" or filter, a missing number), found by a
//! table of checks between each clause and the abilities that stand for it; read-only, no model, no database.
//!
//! `mutation-score` measures the same checks, and the read back of the checker, on single-fault mutants of corpus cards.

pub mod checks;
mod score;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mtg_cards::card::Card;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::check_exact;
use mtg_cards::read::{RowState, sheet};
use mtg_cards::support::PrintedCard;
use mtg_testkit::corpus::printed_card;

use crate::args::SemanticAuditArgs;
use crate::bulk::open_cards;
use crate::cache::Cache;
use crate::draft_suspects::printed_of;
use crate::drafts::{DEFAULT_DIR, FIXTURES};
use crate::error::CliError;
use crate::popularity::Popularity;
use crate::w2a_promote::fixture::w2a_oracle_cards;

pub use score::{MutationScore, mutation_score, score};

/// The corpus rules directory.
pub const CORPUS_DIR: &str = "crates/mtg-cards/cards";
/// The corpus card records.
pub const CORPUS_FIXTURES: &str = "crates/mtg-cards/tests/fixtures/corpus";

/// A card the audit reads: where its rules come from, the rules and the printed card.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The file name without `.json`.
    pub slug: String,
    /// `corpus`, `draft` or `forge`.
    pub source: &'static str,
    /// The rules.
    pub card: Card,
    /// The printed card.
    pub printed: PrintedCard,
}

/// One finding: the check, the clause it read and what differs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    /// The check's name.
    pub check: &'static str,
    /// The clause as printed.
    pub clause: String,
    /// What differs.
    pub note: String,
}

/// The findings of one card: every check on every clause an ability stands for, and `unclaused` for a clause no ability
/// stands for and no by-hand clause covers.
pub fn audit(card: &Card, printed: &PrintedCard) -> Vec<Finding> {
    let names: Vec<String> = std::iter::once(printed.name.clone())
        .chain(printed.faces.iter().map(|f| f.name.clone()))
        .collect();
    let mut found = Vec::new();
    for row in sheet(card, printed).rows {
        match row.state {
            RowState::ByHand { reason: None } => {
                found.push(Finding {
                    check: checks::UNCLAUSED,
                    clause: row.printed.clone(),
                    note: "no ability and no by-hand clause".into(),
                });
                continue;
            }
            RowState::ByHand { .. } | RowState::NoClause => continue,
            _ => {}
        }
        let rules: Vec<serde_json::Value> = row
            .abilities
            .iter()
            .filter_map(|i| card.faces.get(row.face)?.abilities.get(*i))
            .filter_map(|ability| serde_json::to_value(ability).ok())
            .collect();
        if rules.is_empty() {
            continue;
        }
        let refs: Vec<&serde_json::Value> = rules.iter().collect();
        let facts = checks::facts(&refs);
        let text = checks::clause_text(&row.printed, &names);
        for (check, note) in checks::run(&text, &facts) {
            found.push(Finding {
                check,
                clause: row.printed.clone(),
                note,
            });
        }
    }
    found.sort();
    found.dedup();
    found
}

/// The `*.json` file stems of a directory, sorted.
fn stems(dir: &Path) -> Result<Vec<String>, CliError> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map_err(|source| CliError::io(dir, source))?
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_suffix(".json")
                .map(str::to_owned)
        })
        .collect();
    out.sort();
    Ok(out)
}

/// Reads a rules file.
fn read_card(path: &Path) -> Result<Card, CliError> {
    let text = std::fs::read_to_string(path).map_err(|source| CliError::io(path, source))?;
    serde_json::from_str(&text).map_err(|e| CliError::Config(format!("{}: {e}", path.display())))
}

/// The corpus cards: every rules file of `cards/` beside its fixture.
pub fn corpus(cards: &Path, fixtures: &Path) -> Result<Vec<Entry>, CliError> {
    let mut out = Vec::new();
    for stem in stems(cards)? {
        let fixture = fixtures.join(format!("{stem}.json"));
        if !fixture.exists() {
            continue;
        }
        out.push(Entry {
            card: read_card(&cards.join(format!("{stem}.json")))?,
            printed: printed_of(&fixture)?,
            slug: stem,
            source: "corpus",
        });
    }
    Ok(out)
}

/// The drafts of the read tier beside their fixtures.
pub fn drafts(dir: &Path) -> Result<Vec<Entry>, CliError> {
    let mut out = Vec::new();
    for stem in stems(dir)? {
        let Ok(card) = read_card(&dir.join(format!("{stem}.json"))) else {
            continue;
        };
        out.push(Entry {
            card,
            printed: printed_of(&dir.join(FIXTURES).join(format!("{stem}.json")))?,
            slug: stem,
            source: "draft",
        });
    }
    Ok(out)
}

/// The slugs of a translator run's whole, offered drafts that no corpus or compiled Verified rules shadow.
pub fn offered(cards_tsv: &str) -> BTreeSet<String> {
    cards_tsv
        .lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            let shadowed = matches!(cols.get(3), Some(&"corpus-verified" | &"compiled-verified"));
            (cols.get(2) == Some(&"whole") && cols.get(6) == Some(&"true") && !shadowed)
                .then(|| cols[0].to_owned())
        })
        .collect()
}

/// The items mapped on a few threads, in their order.
pub fn par_map<T: Sync, U: Send>(items: &[T], f: impl Fn(&T) -> U + Sync) -> Vec<U> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(6));
    let size = items.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(size)
            .map(|chunk| scope.spawn(|| chunk.iter().map(&f).collect::<Vec<U>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

/// The translator's offered drafts of a run directory that `keep` keeps, completed against their printed cards.
pub fn forge(
    dir: &Path,
    printed: &BTreeMap<String, PrintedCard>,
    keep: &(dyn Fn(&str) -> bool + Sync),
) -> Result<Vec<Entry>, CliError> {
    let tsv_path = dir.join("translate-cards.tsv");
    let tsv =
        std::fs::read_to_string(&tsv_path).map_err(|source| CliError::io(&tsv_path, source))?;
    let caps = mtg_engine::caps();
    let (library, names) = (Library::default(), BTreeSet::new());
    let stems: Vec<String> = offered(&tsv).into_iter().filter(|s| keep(s)).collect();
    let entries = par_map(&stems, |stem| {
        let card = printed.get(stem)?;
        let text = std::fs::read_to_string(dir.join(format!("{stem}.json"))).ok()?;
        let rules = check_exact(&text, card, &library, &names, &caps).rules?;
        Some(Entry {
            slug: stem.clone(),
            source: "forge",
            card: rules,
            printed: card.clone(),
        })
    });
    Ok(entries.into_iter().flatten().collect())
}

/// The printed cards of an Oracle Cards file by slug.
pub fn printed_by_slug(oracle: &Path) -> Result<BTreeMap<String, PrintedCard>, CliError> {
    let mut out = BTreeMap::new();
    for record in open_cards(oracle)?.flatten() {
        let printed = printed_card(&record);
        out.entry(slug(&printed.name)).or_insert(printed);
    }
    Ok(out)
}

/// The newest translator run with a cards table under the proposals directory.
fn newest_forge(cache: &Cache) -> Option<PathBuf> {
    let root = cache.path("proposals");
    let mut runs: Vec<(u32, PathBuf)> = std::fs::read_dir(&root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            let n: u32 = name.strip_prefix("forge")?.parse().ok()?;
            e.path()
                .join("translate-cards.tsv")
                .exists()
                .then(|| (n, e.path()))
        })
        .collect();
    runs.sort();
    runs.pop().map(|(_, path)| path)
}

/// The TSV: slug, check, clause, note, ranked by how many findings the check has, then by popularity (EDHREC rank).
pub fn tsv(rows: &[(&Entry, Finding)], popularity: Option<&Popularity>) -> String {
    let mut per_check: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, finding) in rows {
        *per_check.entry(finding.check).or_default() += 1;
    }
    let rank = |e: &Entry| {
        popularity
            .and_then(|p| p.rank(&e.printed.oracle_id))
            .unwrap_or(u32::MAX)
    };
    let mut sorted: Vec<&(&Entry, Finding)> = rows.iter().collect();
    sorted.sort_by(|(ea, a), (eb, b)| {
        per_check[b.check]
            .cmp(&per_check[a.check])
            .then(a.check.cmp(b.check))
            .then(rank(ea).cmp(&rank(eb)))
            .then(ea.slug.cmp(&eb.slug))
    });
    let mut text = String::from("slug\tcheck\tclause\tnote\n");
    for (entry, finding) in sorted {
        let rank = rank(entry);
        let rank = if rank == u32::MAX {
            "-".to_owned()
        } else {
            rank.to_string()
        };
        let clause = finding.clause.replace(['\t', '\n'], " ");
        let _ = writeln!(
            text,
            "{}\t{}\t{clause}\t{} ({}, rank {rank})",
            entry.slug, finding.check, finding.note, entry.source
        );
    }
    text
}

/// The counts: cards per source, cards with findings, findings per check and source.
pub fn summary(entries: &[Entry], rows: &[(&Entry, Finding)]) -> String {
    let mut per: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut checks_seen = BTreeSet::new();
    for (entry, finding) in rows {
        *per.entry((finding.check, entry.source)).or_default() += 1;
        checks_seen.insert(finding.check);
    }
    let sources = ["corpus", "draft", "forge"];
    let cards = |s: &str| entries.iter().filter(|e| e.source == s).count();
    let flagged = |s: &str| {
        rows.iter()
            .filter(|(e, _)| e.source == s)
            .map(|(e, _)| e.slug.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    };
    let mut text = String::from("source\tcards\tflagged\n");
    for s in sources {
        let _ = writeln!(text, "{s}\t{}\t{}", cards(s), flagged(s));
    }
    text.push_str("check\tcorpus\tdraft\tforge\n");
    for check in checks_seen {
        let n = |s| per.get(&(check, s)).copied().unwrap_or(0);
        let _ = writeln!(
            text,
            "{check}\t{}\t{}\t{}",
            n("corpus"),
            n("draft"),
            n("forge")
        );
    }
    text
}

/// `semantic-audit`: reads the corpus, the drafts and the newest translator run, writes the findings TSV and returns the
/// summary.
pub fn semantic_audit(cache: &Cache, args: &SemanticAuditArgs) -> Result<String, CliError> {
    let oracle = match &args.oracle {
        Some(path) => path.clone(),
        None => w2a_oracle_cards(cache)?,
    };
    let keep = |slug: &str| args.cards.is_empty() || args.cards.iter().any(|c| c == slug);
    let mut entries = corpus(Path::new(CORPUS_DIR), Path::new(CORPUS_FIXTURES))?;
    entries.extend(drafts(Path::new(DEFAULT_DIR))?);
    entries.retain(|e| keep(&e.slug));
    let forge_dir = args.forge.clone().or_else(|| newest_forge(cache));
    if let Some(dir) = &forge_dir {
        entries.extend(forge(dir, &printed_by_slug(&oracle)?, &keep)?);
    }
    let popularity = Popularity::read(&oracle).ok();
    let found = par_map(&entries, |entry| audit(&entry.card, &entry.printed));
    let rows: Vec<(&Entry, Finding)> = entries
        .iter()
        .zip(found)
        .flat_map(|(entry, found)| found.into_iter().map(move |f| (entry, f)))
        .collect();
    let table = tsv(&rows, popularity.as_ref());
    let sum = summary(&entries, &rows);
    let forge_line = forge_dir.map_or_else(
        || "no translator run".to_owned(),
        |d| d.display().to_string(),
    );
    let Some(path) = &args.out else {
        return Ok(format!("{table}\n{sum}{forge_line}"));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::io(parent, source))?;
    }
    std::fs::write(path, &table).map_err(|source| CliError::io(path, source))?;
    Ok(format!(
        "{sum}translator run {forge_line}\nwritten {}",
        path.display()
    ))
}
