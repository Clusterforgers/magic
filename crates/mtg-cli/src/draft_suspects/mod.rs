//! `draft-suspects`: the drafts whose rules, rendered to English by the engine, differ from the Oracle text in a
//! meaning-bearing token (a count, a scope, a zone, a keyword); a deterministic pre-filter, no model, no database.

mod tokens;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mtg_cards::card::Card;
use mtg_cards::read::{ReadRow, RowState, sheet};
use mtg_cards::support::PrintedCard;
use mtg_import::scryfall::cards_jsonl;
use mtg_testkit::corpus::printed_card;

use crate::args::DraftSuspectsArgs;
use crate::drafts::{DEFAULT_DIR, FIXTURES};
use crate::error::CliError;

pub use tokens::{compare, count, findings, normalise};

/// One draft's result: its slug and findings, or why it could not be rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suspect {
    /// The draft's file name without `.json`.
    pub slug: String,
    /// What differs, in the order of the faces; `unrendered` when the renderer cannot word a rule.
    pub findings: Vec<String>,
    /// Whether the draft rendered at all (it loaded and had a card record).
    pub rendered: bool,
}

/// The card record of a fixture file as the strings the renderer reads.
fn printed_of(path: &Path) -> Result<PrintedCard, CliError> {
    let text = std::fs::read_to_string(path).map_err(|source| CliError::io(path, source))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| CliError::Config(format!("{}: {error}", path.display())))?;
    let line = value.to_string();
    let record = cards_jsonl(line.as_bytes())
        .next()
        .and_then(Result::ok)
        .ok_or_else(|| CliError::Config(format!("{}: not a card record", path.display())))?;
    Ok(printed_card(&record))
}

/// The findings of one face: the renderer's wording of the clauses it can word, compared both ways, and the walk of the
/// clauses it cannot word, compared one way (what the Oracle says and the walk lacks), since a walk is structural.
fn face_findings(printed: &PrintedCard, face: usize, rows: &[ReadRow]) -> Vec<String> {
    let (mut worded, mut worded_en) = (String::new(), String::new());
    let (mut walked, mut walked_en) = (String::new(), String::new());
    let mut found = Vec::new();
    for row in rows.iter().filter(|row| row.face == face && !row.continues) {
        match (&row.state, &row.wording) {
            (RowState::Written, _) => {}
            (RowState::ByHand { .. } | RowState::NoClause, _) => {
                found.push("unrendered".to_owned());
            }
            (_, None) => {
                found.push("unrendered".to_owned());
                let _ = write!(walked, " {}", row.printed);
                for step in &row.steps {
                    let _ = write!(walked_en, " {}.", step.text);
                }
            }
            (_, Some(wording)) => {
                let _ = write!(worded, " {}", row.printed);
                let _ = write!(worded_en, " {wording}");
            }
        }
    }
    let names: Vec<String> = std::iter::once(printed.name.clone())
        .chain(printed.faces.iter().map(|f| f.name.clone()))
        .collect();
    let keywords: Vec<String> = printed.keywords.iter().map(|k| k.to_lowercase()).collect();
    found.extend(findings(&worded, &worded_en, &names, &keywords));
    found.extend(
        findings(&walked, &walked_en, &names, &keywords)
            .into_iter()
            .filter(|finding| finding.starts_with("missing:")),
    );
    found
}

/// Renders one draft against its card and compares the English with the Oracle text.
pub fn suspect(slug: &str, draft: &str, printed: &PrintedCard) -> Suspect {
    let Ok(card) = serde_json::from_str::<Card>(draft) else {
        return Suspect {
            slug: slug.to_owned(),
            findings: vec!["unloadable".to_owned()],
            rendered: false,
        };
    };
    let shown = sheet(&card, printed);
    let mut found = Vec::new();
    for face in 0..printed.faces.len() {
        found.extend(face_findings(printed, face, &shown.rows));
    }
    if !shown.extra.is_empty() {
        found.push("noclause".to_owned());
    }
    found.sort();
    found.dedup();
    let rendered = !found.iter().any(|f| f == "unrendered");
    Suspect {
        slug: slug.to_owned(),
        findings: found,
        rendered,
    }
}

/// The drafts of the arguments (every `*.json` of the directory, or the one `--file`) judged.
pub fn run(args: &DraftSuspectsArgs) -> Result<Vec<Suspect>, CliError> {
    let dir = args
        .dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DIR));
    let mut slugs: Vec<String> = match &args.file {
        Some(file) => vec![file.trim_end_matches(".json").to_owned()],
        None => std::fs::read_dir(&dir)
            .map_err(|source| CliError::io(&dir, source))?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry
                    .file_name()
                    .to_str()?
                    .strip_suffix(".json")
                    .map(str::to_owned)
            })
            .collect(),
    };
    slugs.sort();
    let mut out = Vec::new();
    for slug in slugs {
        let file = format!("{slug}.json");
        let text = std::fs::read_to_string(dir.join(&file))
            .map_err(|s| CliError::io(dir.join(&file), s))?;
        let printed = printed_of(&dir.join(FIXTURES).join(&file))?;
        out.push(suspect(&slug, &text, &printed));
    }
    Ok(out)
}

/// The TSV: a header, then `slug`, score and findings for each draft, the highest score first.
pub fn tsv(suspects: &[Suspect]) -> String {
    let mut sorted: Vec<&Suspect> = suspects.iter().collect();
    sorted.sort_by(|a, b| {
        b.findings
            .len()
            .cmp(&a.findings.len())
            .then(a.slug.cmp(&b.slug))
    });
    let mut text = String::from("slug\tscore\tfindings\n");
    for s in sorted {
        let _ = writeln!(
            text,
            "{}\t{}\t{}",
            s.slug,
            s.findings.len(),
            s.findings.join("; ")
        );
    }
    text
}

/// The counts: drafts, how many render, and how many of those have 0, 1, 2 and 3 or more findings.
pub fn summary(suspects: &[Suspect]) -> String {
    let rendered: Vec<&Suspect> = suspects.iter().filter(|s| s.rendered).collect();
    let at = |n: usize| rendered.iter().filter(|s| s.findings.len() == n).count();
    let many = rendered.iter().filter(|s| s.findings.len() >= 3).count();
    let content = |s: &Suspect| {
        s.findings
            .iter()
            .filter(|f| *f != "unrendered" && *f != "unloadable")
            .count()
    };
    let by = |n: usize| suspects.iter().filter(|s| content(s) == n).count();
    let many_content = suspects.iter().filter(|s| content(s) >= 3).count();
    format!(
        "{} drafts, {} render whole, {} partly or not (unrendered, unloadable)\nscore (all findings) of the drafts that render whole: 0 {}, 1 {}, 2 {}, 3+ {}\ncontent findings (without unrendered) of all drafts: 0 {}, 1 {}, 2 {}, 3+ {}",
        suspects.len(),
        rendered.len(),
        suspects.len() - rendered.len(),
        at(0),
        at(1),
        at(2),
        many,
        by(0),
        by(1),
        by(2),
        many_content
    )
}

/// `draft-suspects`: writes the TSV when `--out` names a file; returns the summary, or the TSV and summary without `--out`.
pub fn draft_suspects(args: &DraftSuspectsArgs) -> Result<String, CliError> {
    let suspects = run(args)?;
    let table = tsv(&suspects);
    let sum = summary(&suspects);
    let Some(path) = &args.out else {
        return Ok(format!("{table}\n{sum}"));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::io(parent, source))?;
    }
    std::fs::write(path, &table).map_err(|source| CliError::io(path, source))?;
    Ok(format!("{sum}\nwritten {}", path.display()))
}
