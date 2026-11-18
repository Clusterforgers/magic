//! `export-precon`: a stored precon as a content deck file of `mtg-campaign` (`content/decks/<code>-<slug>.json`):
//! card names and counts, the face commander shuffled in as an ordinary card, the listed swaps applied. Read-only.

use std::path::{Path, PathBuf};

use mtg_db::{Database, DeckBoard, PreconDeckDetail, PreconEntry};
use serde::Serialize;

use crate::args::ExportPreconArgs;
use crate::error::CliError;

/// One swap: every copy of `out` leaves the deck and as many copies of `into` take its place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Swap {
    /// The card that leaves.
    pub out: String,
    /// The card that takes its place.
    pub into: String,
}

impl Swap {
    /// Reads `Old card=New card`.
    pub fn parse(text: &str) -> Result<Swap, CliError> {
        let (out, into) = text
            .split_once('=')
            .map(|(out, into)| (out.trim(), into.trim()))
            .filter(|(out, into)| !out.is_empty() && !into.is_empty())
            .ok_or_else(|| {
                CliError::Config(format!("a swap reads `Old card=New card`, not `{text}`"))
            })?;
        Ok(Swap {
            out: out.to_owned(),
            into: into.to_owned(),
        })
    }
}

/// A content deck file: the precon it comes from, its face commander, the swaps made and the cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeckFile {
    /// The set code of the precon.
    pub code: String,
    /// The precon's name.
    pub name: String,
    /// The precon's face commander, shuffled into the cards.
    pub commander: Vec<String>,
    /// The swaps made, in the order given.
    pub swaps: Vec<Swap>,
    /// The cards and their counts, in deck order; a name appears once.
    pub cards: Vec<(String, u32)>,
}

impl DeckFile {
    /// The number of cards.
    pub fn size(&self) -> u32 {
        self.cards.iter().map(|(_, count)| count).sum()
    }
}

/// The name an entry plays under: the stored card's, else the deck file's spelling.
fn entry_name(entry: &PreconEntry) -> &str {
    entry
        .card
        .as_ref()
        .map_or(entry.card_name.as_str(), |card| card.name.as_str())
}

/// The deck file of a precon: commander and main board (no sideboard) as one list, the swaps applied. A swap whose
/// card is not in the deck is refused.
pub fn deck_file(detail: &PreconDeckDetail, swaps: &[Swap]) -> Result<DeckFile, CliError> {
    let mut cards: Vec<(String, u32)> = Vec::new();
    let mut commander = Vec::new();
    for entry in &detail.entries {
        match entry.board {
            DeckBoard::Sideboard => continue,
            DeckBoard::Commander => commander.push(entry_name(entry).to_owned()),
            DeckBoard::Main => {}
        }
        add(&mut cards, entry_name(entry), entry.count);
    }
    for swap in swaps {
        let at = cards
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(&swap.out))
            .ok_or_else(|| {
                CliError::Config(format!(
                    "{}: no `{}` to swap",
                    detail.summary.name, swap.out
                ))
            })?;
        let (_, count) = cards.remove(at);
        add(&mut cards, &swap.into, count);
    }
    Ok(DeckFile {
        code: detail.summary.code.clone(),
        name: detail.summary.name.clone(),
        commander,
        swaps: swaps.to_vec(),
        cards,
    })
}

/// Adds `count` copies of `name`, to its line when it has one.
fn add(cards: &mut Vec<(String, u32)>, name: &str, count: u32) {
    match cards.iter_mut().find(|(have, _)| have == name) {
        Some((_, have)) => *have += count,
        None => cards.push((name.to_owned(), count)),
    }
}

/// The file name of a deck: the code in lower case, then the name's slug with dashes (`blc-squirreled-away.json`).
pub fn file_name(code: &str, name: &str) -> String {
    let slug = mtg_cards::load::slug(name).replace('_', "-");
    format!("{}-{slug}.json", code.to_lowercase())
}

/// The deck file as written: pretty JSON, a final newline.
pub fn render(deck: &DeckFile) -> Result<String, CliError> {
    serde_json::to_string_pretty(deck)
        .map(|text| text + "\n")
        .map_err(|error| CliError::Config(format!("the deck file writes: {error}")))
}

/// Runs `export-precon`: finds the precon by code and name, writes its deck file into `--out` and says where.
pub async fn export_precon(db: &Database, args: &ExportPreconArgs) -> Result<String, CliError> {
    let swaps = args
        .swap
        .iter()
        .map(|text| Swap::parse(text))
        .collect::<Result<Vec<_>, _>>()?;
    let repo = db.precons();
    let summary = repo
        .list()
        .await?
        .into_iter()
        .find(|deck| deck.code.eq_ignore_ascii_case(&args.code) && deck.name == args.name)
        .ok_or_else(|| CliError::Config(format!("no stored precon {} {}", args.code, args.name)))?;
    let detail = repo
        .get(summary.id)
        .await?
        .ok_or_else(|| CliError::Config(format!("precon {} {} vanished", args.code, args.name)))?;
    let deck = deck_file(&detail, &swaps)?;
    let path = write(&args.out, &deck)?;
    Ok(format!("{}: {} cards", path.display(), deck.size()))
}

/// `export-precon` from the command line: connects, then [`export_precon`].
pub async fn command(
    database_url: Option<String>,
    args: &ExportPreconArgs,
) -> Result<String, CliError> {
    export_precon(&crate::connect(database_url).await?, args).await
}

/// Writes the deck file into `dir` under [`file_name`].
pub fn write(dir: &Path, deck: &DeckFile) -> Result<PathBuf, CliError> {
    std::fs::create_dir_all(dir).map_err(|source| CliError::io(dir, source))?;
    let path = dir.join(file_name(&deck.code, &deck.name));
    std::fs::write(&path, render(deck)?).map_err(|source| CliError::io(&path, source))?;
    Ok(path)
}
