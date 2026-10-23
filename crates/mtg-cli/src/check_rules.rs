//! `check-rules`: proposed rules files (written by hand or by a model) checked against the stored cards without trusting
//! them: valid, read back as the printed text, and the support state they give (`docs/cards-dsl.md`, Model-assisted reading).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use mtg_cards::builtin;
use mtg_cards::load::{canonical, slug};
use mtg_cards::proposal::{Report, Verdict, check};
use mtg_cards::{Caps, PrintedCard};
use mtg_db::Database;

use crate::args::CheckRulesArgs;
use crate::audit::stored_cards;
use crate::compile::names_of;
use crate::error::CliError;

/// The checked proposals of a run, in the order of the files.
#[derive(Debug)]
pub struct Checked {
    /// One report for each file.
    pub reports: Vec<Report>,
    /// The files written with `--write`.
    pub written: Vec<PathBuf>,
}

impl Checked {
    /// The reports as text, then a line of counts by summary when there is more than one file.
    pub fn render(&self, verbose: bool) -> String {
        let mut text = self
            .reports
            .iter()
            .map(|report| report.render(verbose))
            .collect::<Vec<_>>()
            .join("\n");
        if self.reports.len() > 1 {
            let mut counts = BTreeMap::new();
            for report in &self.reports {
                *counts.entry(report.summary()).or_insert(0usize) += 1;
            }
            let tally: Vec<String> = counts.iter().map(|(k, n)| format!("{n} {k}")).collect();
            let _ = write!(text, "\n{} cards: {}", self.reports.len(), tally.join(", "));
        }
        for path in &self.written {
            let _ = write!(text, "\nwritten {}", path.display());
        }
        text
    }
}

/// The stored card a file is for: the named card, or the one whose slug is the file's name.
fn card_of<'a>(
    cards: &'a [PrintedCard],
    wanted: Option<&str>,
    file: &Path,
) -> Result<&'a PrintedCard, CliError> {
    let stem = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    let lower = wanted.map(str::to_lowercase);
    let found = cards.iter().find(|card| match &lower {
        Some(lower) => {
            card.name.to_lowercase() == *lower
                || card
                    .faces
                    .iter()
                    .any(|face| face.name.to_lowercase() == *lower)
        }
        None => slug(&card.name) == stem,
    });
    found.ok_or_else(|| {
        CliError::Config(match wanted {
            Some(name) => format!("no stored card is named `{name}`"),
            None => format!(
                "{}: no stored card has this file name as the slug of its name; name the card with --card",
                file.display()
            ),
        })
    })
}

/// The stored card and the JSON text of each proposal of the arguments: one for each file, or one for each entry of a bundle.
async fn proposals<'a>(
    cards: &'a [PrintedCard],
    args: &CheckRulesArgs,
) -> Result<Vec<(&'a PrintedCard, String)>, CliError> {
    let mut found = Vec::new();
    for file in &args.files {
        let text = tokio::fs::read_to_string(file)
            .await
            .map_err(|source| CliError::io(file, source))?;
        if !args.bundle {
            found.push((card_of(cards, args.card.as_deref(), file)?, text));
            continue;
        }
        let entries: BTreeMap<String, serde_json::Value> = serde_json::from_str(&text)
            .map_err(|error| CliError::Config(format!("{}: {error}", file.display())))?;
        for (name, rules) in entries {
            found.push((card_of(cards, Some(&name), file)?, rules.to_string()));
        }
    }
    Ok(found)
}

/// Checks the files of the arguments against the stored cards.
pub async fn check_rules(db: &Database, args: &CheckRulesArgs) -> Result<Checked, CliError> {
    if args.card.is_some() && args.files.len() != 1 {
        return Err(CliError::Config(
            "--card names the card of exactly one file".into(),
        ));
    }
    let hand_written = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let cards = stored_cards(db).await?;
    let names: BTreeSet<String> = names_of(&cards);
    let caps: Caps = mtg_engine::caps();
    let mut checked = Checked {
        reports: Vec::new(),
        written: Vec::new(),
    };
    for (printed, text) in proposals(&cards, args).await? {
        let report = check(&text, printed, hand_written, &names, &caps);
        if let (Some(dir), Some(rules)) = (&args.write, &report.rules)
            && matches!(report.verdict(), Verdict::Accepted | Verdict::Unchecked)
        {
            let path = dir.join(format!("{}.json", slug(&printed.name)));
            tokio::fs::write(&path, canonical(rules))
                .await
                .map_err(|source| CliError::io(&path, source))?;
            checked.written.push(path);
        }
        checked.reports.push(report);
    }
    Ok(checked)
}
