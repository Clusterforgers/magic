//! `translate-forge`: Forge card scripts, read at run time from an owner-provided checkout, translated by mappers into
//! proposals in the format `propose` takes from a model (`docs/cards-dsl.md`, Model-assisted reading).
//!
//! The translator never guesses: a Forge construct without a mapper leaves its line out of the proposal (the checker then
//! sees the printed clause unwritten and marks the card `Partial`), and every ability it writes parses as the DSL's
//! `Ability`. Nothing of a script is copied into the repository; the proposals are data derived from the scripts.

pub mod ability;
pub mod amount;
pub mod card;
pub mod condition;
pub mod cost;
pub mod ctx;
pub mod effects;
pub mod effects_effect;
pub mod effects_loop;
pub mod effects_obj;
pub mod effects_zone;
pub mod keyword;
pub mod mayplay;
pub mod needs;
pub mod oracle;
pub mod replacements;
pub mod replacements_damage;
pub mod report;
pub mod statics;
pub mod stored;
pub mod tokens;
pub mod triggers;
pub mod triggers_more;
pub mod valid;
pub mod valid_atoms;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use mtg_cards::load::slug;
use serde_json::{Value, json};

use mtg_db::Database;

use crate::args::TranslateForgeArgs;
use crate::cache::Cache;
use crate::error::CliError;
use crate::forge::script::{Face, Script, fold, read_checkout};
use crate::popularity::Popularity;
use card::{CardOut, translate_script};
use ctx::World;
use report::Funnel;
use stored::Stored;

/// Reads the checkout, translates the selected cards and writes a proposal for each card with something translated.
pub async fn translate_forge(
    cache: &Cache,
    db: Option<&Database>,
    args: &TranslateForgeArgs,
) -> Result<String, CliError> {
    let stored = match db {
        Some(db) => Some(Stored::load(db, args.precons).await?),
        None => None,
    };
    let scripts = read_checkout(&args.dir)?;
    let world = World::new(&scripts, read_tokens(&args.dir));
    let wanted: Vec<String> = args.cards.iter().map(|name| fold(name)).collect();
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| cache.path("proposals/forge"));
    let mut funnel = Funnel::default();
    let popularity = Popularity::from_cache(cache).ok();
    let mut index = String::from("slug\tname\ttranslated\ttoday\n");
    let mut cards = String::from("slug\tname\ttranslated\ttoday\toracle_matches\tgaps\n");
    for script in &scripts {
        if !wanted.is_empty() && !selected(script, &wanted) {
            continue;
        }
        let card = translate_script(&world, script);
        let stem = slug(&card.name);
        if stored.as_ref().is_some_and(|stored| !stored.has(&stem)) {
            funnel.unstored += 1;
            continue;
        }
        let today = match &stored {
            Some(stored) => stored.today(&stem),
            _ => String::new(),
        };
        let weight = match (&stored, &popularity) {
            (Some(stored), Some(ranks)) => {
                stored.oracle_id(&stem).map_or(0.0, |id| ranks.weight(id))
            }
            _ => 0.0,
        };
        let oracle_matches = stored.as_ref().and_then(|stored| {
            let printed = stored.printed_oracle(&stem)?;
            Some(oracle::matches(
                &card.name,
                card.oracle.iter().map(String::as_str),
                printed,
            ))
        });
        let written = funnel.add(&card, !today.ends_with("verified"), weight, &today);
        let state = match (card.lines(), card.translated(), card.gaps().next()) {
            (0, _, _) => "plain",
            (_, 0, _) => "skipped",
            (_, _, None) => "whole",
            _ => "partial",
        };
        let gaps: BTreeSet<&str> = card.gaps().map(|gap| gap.0.as_str()).collect();
        let gaps: Vec<&str> = gaps.into_iter().collect();
        let _ = writeln!(
            cards,
            "{stem}\t{}\t{state}\t{today}\t{}\t{}",
            card.name,
            oracle_matches.map_or("", |same| if same { "true" } else { "false" }),
            gaps.join("; ")
        );
        if written {
            write_proposal(&out_dir, &card, oracle_matches)?;
            let _ = writeln!(index, "{stem}\t{}\t{state}\t{today}", card.name);
        }
    }
    let ledger = out_dir.join("translate-gaps.tsv");
    write_file(&ledger, &funnel.ledger())?;
    write_file(&out_dir.join("engine-needs.tsv"), &funnel.engine_needs())?;
    write_file(&out_dir.join("translate-index.tsv"), &index)?;
    write_file(&out_dir.join("translate-cards.tsv"), &cards)?;
    if let Some(lines) = stored.as_ref().and_then(Stored::precon_lines) {
        write_file(&out_dir.join("precon-today.tsv"), &lines)?;
    }
    Ok(funnel.render(&out_dir))
}

/// Whether a script is one of the named cards (by the card's name or the name of a face).
fn selected(script: &Script, wanted: &[String]) -> bool {
    let names: Vec<&str> = script.names().collect();
    let joined = names.join(" // ");
    std::iter::once(joined.as_str())
        .chain(names)
        .any(|name| wanted.contains(&fold(name)))
}

/// Writes the proposal of a card as `<slug>.json` and marks its source in `meta/<slug>.json`.
fn write_proposal(
    dir: &Path,
    card: &CardOut,
    oracle_matches: Option<bool>,
) -> Result<(), CliError> {
    let stem = slug(&card.name);
    let text = serde_json::to_string_pretty(&card.proposal()).unwrap_or_default();
    write_file(&dir.join(format!("{stem}.json")), &format!("{text}\n"))?;
    let meta = dir.join("meta").join(format!("{stem}.json"));
    let mut record = fs::read_to_string(&meta)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    record["source"] = json!("forge");
    if let Some(matches) = oracle_matches {
        record["oracle_matches"] = json!(matches);
    }
    let text = serde_json::to_string_pretty(&record).unwrap_or_default();
    write_file(&meta, &format!("{text}\n"))
}

fn write_file(path: &Path, text: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::io(parent, source))?;
    }
    fs::write(path, text).map_err(|source| CliError::io(path, source))
}

/// The token scripts next to the card scripts of a checkout, by file stem; none when the folder is not found.
fn read_tokens(dir: &Path) -> BTreeMap<String, Face> {
    let candidates: Vec<PathBuf> = vec![
        dir.join("forge-gui/res/tokenscripts"),
        dir.join("tokenscripts"),
        dir.join("../tokenscripts"),
    ];
    let mut tokens = BTreeMap::new();
    let Some(folder) = candidates.iter().find(|path| path.is_dir()) else {
        return tokens;
    };
    let Ok(entries) = fs::read_dir(folder) else {
        return tokens;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if path.extension().is_none_or(|extension| extension != "txt") {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let script = Script::parse(&String::from_utf8_lossy(&bytes));
        if let Some(face) = script.faces.into_iter().next() {
            tokens.insert(stem.to_owned(), face);
        }
    }
    tokens
}
