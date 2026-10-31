//! `forge-check`: a cross-check of the compiler and the hand-written rules against Forge's card scripts, read at run
//! time from a local checkout (`docs/reference-engines.md`).
//!
//! Nothing of a script is stored: the report holds card names, the names of Forge's APIs, modes and keys, and counts.

pub mod compare;
pub mod map;
pub mod report;
pub mod script;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use mtg_cards::builtin;
use mtg_cards::compile::compile;
use mtg_cards::coverage::{Blocker, PreconCard, RulesSource, analyze_with};
use mtg_cards::load::Library;
use mtg_cards::{Caps, PrintedCard};
use mtg_db::Database;

use crate::args::ForgeCheckArgs;
use crate::coverage::precon_cards;
use crate::error::CliError;
use compare::{compare, verbs_of};
use map::Map;
use report::{CardCheck, Rows, Survey};
use script::{Script, fold, read_checkout};

/// Reads the scripts of the checkout, compares them with the rules of the stored precon cards and returns the report: the
/// summary, and with `--out` the whole report is written to the file.
pub async fn forge_check(db: &Database, args: &ForgeCheckArgs) -> Result<String, CliError> {
    let scripts = read_checkout(&args.forge)?;
    let map = Map::standard()?;
    let (cards, _) = precon_cards(db, args.deck.as_deref()).await?;
    let library = builtin::library().map_err(|errors| {
        CliError::Config(format!("the embedded rules files do not load:\n{errors}"))
    })?;
    let survey = survey(&cards, &scripts, map, library, &mtg_engine::caps());
    let rows = Rows {
        top: args.top,
        keys: args.keys,
    };
    let summary = survey.render(rows, false);
    let Some(path) = &args.out else {
        return Ok(summary);
    };
    tokio::fs::write(path, format!("{}\n", survey.render(rows, true)))
        .await
        .map_err(|source| CliError::io(path, source))?;
    Ok(format!(
        "{summary}\nforge-check: written to {}",
        path.display()
    ))
}

/// Sets the script of every card that Forge has against the rules of the card: the hand-written file, else the
/// compiler's reading.
pub fn survey(
    cards: &[PreconCard],
    scripts: &[Script],
    map: &Map,
    library: &Library,
    caps: &Caps,
) -> Survey {
    let index = script_index(scripts);
    let report = analyze_with(cards, library, caps);
    let printed: BTreeMap<&str, &PrintedCard> = cards
        .iter()
        .map(|card| (card.printed.oracle_id.as_str(), &card.printed))
        .collect();
    let mut survey = Survey {
        stored: report.cards.len(),
        ..Survey::default()
    };
    for result in &report.cards {
        let Some(&printed) = printed.get(result.oracle_id.as_str()) else {
            continue;
        };
        let Some(script) = script_of(printed, &index).and_then(|at| scripts.get(at)) else {
            survey.absent.push(result.name.clone());
            continue;
        };
        let invalid = result
            .blockers
            .contains(&Blocker::Rules("invalid".to_owned()));
        let ours = match result.rules {
            RulesSource::HandWritten => library.card(&printed.oracle_id).map(verbs_of),
            RulesSource::Draft => library
                .draft(&printed.oracle_id)
                .map(|draft| verbs_of(&draft.card)),
            RulesSource::Compiled if !invalid => compile(printed).card.as_ref().map(verbs_of),
            RulesSource::Compiled | RulesSource::Absent => None,
        }
        .unwrap_or_default();
        survey.checks.push(CardCheck {
            name: result.name.clone(),
            source: result.rules,
            support: result.support,
            unread: result
                .blockers
                .iter()
                .any(|blocker| matches!(blocker, Blocker::Clause(_) | Blocker::Rules(_))),
            findings: compare(&script.items(), &ours, map),
            ours: ours.into_iter().collect(),
        });
    }
    survey
}

/// The scripts by the folded name of each face and of all faces joined as Scryfall names a multi-face card; the first
/// script of a name wins.
fn script_index(scripts: &[Script]) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    for (at, script) in scripts.iter().enumerate() {
        let names: Vec<&str> = script.names().collect();
        for name in &names {
            index.entry(fold(name)).or_insert(at);
        }
        if names.len() > 1 {
            index.entry(fold(&names.join(" // "))).or_insert(at);
        }
    }
    index
}

/// The script of a stored card: by its name, else by the name of a face.
fn script_of(printed: &PrintedCard, index: &BTreeMap<String, usize>) -> Option<usize> {
    std::iter::once(printed.name.as_str())
        .chain(printed.faces.iter().map(|face| face.name.as_str()))
        .find_map(|name| index.get(&fold(name)).copied())
}
