//! `read-candidates`: the drafts of the build marked `differs` whose clauses read back as other text only in wording
//! (`mtg_cards::wording`), as counts per equivalence and a TSV; read-only against the local database's cards.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use mtg_cards::card::Card;
use mtg_cards::proposal::{ClauseState, differing};
use mtg_cards::wording::{ClauseMatch, Rule, bulk_candidate, same_words};
use mtg_cards::{Caps, PrintedCard, builtin};
use mtg_db::Database;

use crate::args::ReadCandidatesArgs;
use crate::audit::stored_cards;
use crate::drafts::{DraftMeta, embedded, embedded_meta, fitting};
use crate::error::CliError;

/// One draft marked `differs` and how its clauses stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    /// The draft's slug (the card's name as a file name).
    pub slug: String,
    /// The card's name.
    pub name: String,
    /// Every clause matched under the equivalences; `None` when one is not.
    pub candidate: Option<Vec<ClauseMatch>>,
    /// The clauses that match no equivalence: printed and engine words.
    pub misses: Vec<(String, String)>,
}

/// Judges the drafts of `files` that fit a card of `cards` and are marked `differs`, in slug order.
pub fn judge(
    files: &[(&str, &str)],
    meta: &[(&str, &str)],
    caps: &Caps,
    cards: &[PrintedCard],
) -> Result<Vec<Judged>, CliError> {
    let library = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let by_id: BTreeMap<&str, &PrintedCard> = cards
        .iter()
        .map(|card| (card.oracle_id.as_str(), card))
        .collect();
    // Only the marked drafts: the fit of the others is `compile`'s business and slow to judge.
    let differs: BTreeSet<&str> = meta
        .iter()
        .filter(|(_, text)| DraftMeta::parse(text).differs)
        .map(|(slug, _)| *slug)
        .collect();
    let marked: Vec<(&str, &str)> = files
        .iter()
        .filter(|(slug, _)| differs.contains(slug))
        .copied()
        .collect();
    let (held, _) = fitting(&marked, meta, library, caps, cards);
    let mut out = Vec::new();
    for draft in held.iter().filter(|draft| draft.differs) {
        let Some(printed) = by_id.get(draft.oracle_id.as_str()) else {
            continue;
        };
        let Ok(card) = serde_json::from_value::<Card>(draft.rules.clone()) else {
            continue;
        };
        let clauses = differing(&card, printed);
        let misses = clauses
            .iter()
            .filter_map(|clause| match &clause.state {
                ClauseState::Differs { written, .. }
                    if same_words(&clause.text, written).is_none() =>
                {
                    Some((clause.text.clone(), written.clone()))
                }
                _ => None,
            })
            .collect();
        out.push(Judged {
            slug: mtg_cards::load::slug(&printed.name),
            name: printed.name.clone(),
            candidate: bulk_candidate(&clauses),
            misses,
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

/// A field of the TSV: tabs and line breaks become spaces.
fn field(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

/// The TSV: a header, then one line per matched clause of each candidate.
pub fn tsv(judged: &[Judged]) -> String {
    let mut text = String::from("slug\tname\tprinted\tengine\trule\n");
    for card in judged {
        for clause in card.candidate.iter().flatten() {
            let rules: Vec<&str> = clause.rules.iter().map(|rule| rule.name()).collect();
            let _ = writeln!(
                text,
                "{}\t{}\t{}\t{}\t{}",
                card.slug,
                field(&card.name),
                field(&clause.printed),
                field(&clause.written),
                rules.join(",")
            );
        }
    }
    text
}

/// The counts: drafts marked `differs`, candidates, and the candidates that need each rule.
pub fn summary(judged: &[Judged]) -> String {
    let candidates: Vec<&Vec<ClauseMatch>> =
        judged.iter().filter_map(|j| j.candidate.as_ref()).collect();
    let mut text = format!(
        "{} drafts marked differs, {} bulk candidates (every clause wording only)\n",
        judged.len(),
        candidates.len()
    );
    for rule in Rule::ALL {
        let cards = candidates
            .iter()
            .filter(|clauses| clauses.iter().any(|clause| clause.rules.contains(rule)))
            .count();
        let _ = writeln!(text, "  {:<16} {cards}", rule.name());
    }
    text
}

/// Writes `text` to `path`, making its folder.
fn write(path: &Path, text: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|source| CliError::io(parent, source))?;
    }
    std::fs::write(path, text).map_err(|source| CliError::io(path, source))
}

/// `read-candidates`: judges the embedded drafts against the stored cards, writes the TSV and returns the summary.
pub async fn read_candidates(db: &Database, args: &ReadCandidatesArgs) -> Result<String, CliError> {
    let cards = stored_cards(db).await?;
    let judged = judge(embedded(), embedded_meta(), &mtg_engine::caps(), &cards)?;
    write(&args.out, &tsv(&judged))?;
    let mut text = summary(&judged);
    if let Some(path) = &args.misses {
        let mut misses = String::from("slug\tprinted\tengine\n");
        for card in &judged {
            for (printed, written) in &card.misses {
                let _ = writeln!(
                    misses,
                    "{}\t{}\t{}",
                    card.slug,
                    field(printed),
                    field(written)
                );
            }
        }
        write(path, &misses)?;
        let _ = writeln!(text, "misses written {}", path.display());
    }
    let _ = write!(text, "written {}", args.out.display());
    Ok(text)
}

#[cfg(test)]
mod tests {
    //! Goal: a draft marked `differs` whose clause differs only in wording is a candidate with its rule, one that drops
    //! a clause is a miss, an unmarked draft is not judged, and the TSV and the counts say so.
    use mtg_import::scryfall::cards_jsonl;
    use mtg_testkit::corpus::printed_card;

    use super::*;

    const KNOWING: &str = include_str!("../drafts/knowing_half_the_battle.json");
    const AETHER: &str = include_str!("../drafts/aether_web.json");
    const MARKED: &str = r#"{"source": "forge", "oracle_matches": true, "differs": true}"#;

    fn printed(record: &str) -> PrintedCard {
        let value: serde_json::Value = serde_json::from_str(record).unwrap();
        let line = value.to_string();
        printed_card(&cards_jsonl(line.as_bytes()).next().unwrap().unwrap())
    }

    #[test]
    fn a_wording_only_draft_is_a_candidate_and_a_dropped_clause_a_miss() {
        let cards = [
            printed(include_str!(
                "../drafts/fixtures/knowing_half_the_battle.json"
            )),
            printed(include_str!("../drafts/fixtures/aether_web.json")),
        ];
        let files = [("aether_web", AETHER), ("knowing_half_the_battle", KNOWING)];
        let meta = [("aether_web", MARKED), ("knowing_half_the_battle", MARKED)];
        let judged = judge(&files, &meta, &mtg_engine::caps(), &cards).unwrap();
        assert_eq!(judged.len(), 2);
        let (aether, knowing) = (&judged[0], &judged[1]);
        assert_eq!(aether.candidate, None);
        assert_eq!(aether.misses.len(), 1);
        let clauses = knowing.candidate.as_ref().unwrap();
        assert_eq!(clauses[0].rules, [Rule::Punctuation]);
        assert_eq!(knowing.misses, Vec::<(String, String)>::new());

        let table = tsv(&judged);
        assert_eq!(table.lines().count(), 2);
        assert!(
            table
                .lines()
                .nth(1)
                .unwrap()
                .starts_with("knowing_half_the_battle\tKnowing // Half the Battle\t")
        );
        assert!(table.trim_end().ends_with("\tpunctuation"));
        let counts = summary(&judged);
        assert!(
            counts.starts_with("2 drafts marked differs, 1 bulk candidates"),
            "{counts}"
        );
        assert!(counts.contains("punctuation"), "{counts}");

        let unmarked = judge(&files, &[], &mtg_engine::caps(), &cards).unwrap();
        assert_eq!(unmarked.len(), 0);
    }
}
