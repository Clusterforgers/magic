//! `propose --forge-hint DIR`: the Forge script lines of a card, read at run time from a read-only checkout and put into
//! the card's prompt as another engine's reading of the same card. Nothing of a script is stored or saved.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use mtg_cards::PrintedCard;
use serde::Serialize;

use crate::error::CliError;
use crate::forge::script::fold;

/// The most script lines the hint of a card holds.
pub const MAX_LINES: usize = 40;

/// The most characters of script lines the hint of a card holds.
pub const MAX_CHARS: usize = 2_500;

/// The note that ends a hint cut at the limits.
pub const TRUNCATED: &str = "(truncated)";

/// The tags of the script lines a hint keeps; the name, cost, types, stats, Oracle text, art and flavor lines are left out.
const KEPT_TAGS: [&str; 6] = ["A", "T", "S", "R", "K", "SVar"];

/// The names of the `SVar` lines that only steer Forge's computer player (`AIPreference`, `PlayMain1`).
fn is_ai_hint(svar: &str) -> bool {
    let name = svar.split(':').next().unwrap_or_default().trim();
    name.starts_with("AI") || name == "PlayMain1"
}

/// Whether a card got a hint in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HintState {
    /// The run has no `--forge-hint`.
    Off,
    /// The prompt holds the lines of the card's script.
    Hinted,
    /// The card has no script, or its script has no line worth showing.
    Unhinted,
}

/// The hint modes a run used, as saved beside each proposal so that an A/B of runs can be told apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HintRecord {
    /// The `--solved-examples` count; 0 means the hand-written examples.
    pub solved_examples: usize,
    /// Whether the card's prompt held a Forge hint.
    pub forge_hint: HintState,
}

impl HintRecord {
    /// Whether the run used no hint mode, so that nothing is saved for it.
    pub fn is_plain(&self) -> bool {
        self.solved_examples == 0 && self.forge_hint == HintState::Off
    }
}

/// The file name stem Forge gives a card: lowercase, accents folded, spaces as `_`, punctuation dropped.
pub fn file_stem(name: &str) -> String {
    let mut stem = String::new();
    for c in fold(name).chars() {
        if c == ' ' {
            if !stem.ends_with('_') {
                stem.push('_');
            }
        } else if c.is_alphanumeric() {
            stem.push(c);
        }
    }
    stem.trim_matches('_').to_owned()
}

/// The `Name:` of every face of a script.
fn names_of(script: &str) -> impl Iterator<Item = &str> {
    script
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Name:"))
        .map(str::trim)
}

/// One face of a script: its name and the lines a hint keeps.
struct Face<'a> {
    name: &'a str,
    lines: Vec<&'a str>,
}

/// The faces of a script, split at `ALTERNATE`, with the kept lines of each.
fn faces_of(script: &str) -> Vec<Face<'_>> {
    let mut faces = vec![Face {
        name: "",
        lines: Vec::new(),
    }];
    for line in script.lines().map(str::trim) {
        let Some(face) = faces.last_mut() else {
            continue;
        };
        if line == "ALTERNATE" {
            faces.push(Face {
                name: "",
                lines: Vec::new(),
            });
            continue;
        }
        let Some((tag, body)) = line.split_once(':') else {
            continue;
        };
        if tag == "Name" {
            face.name = body.trim();
        } else if KEPT_TAGS.contains(&tag) && !(tag == "SVar" && is_ai_hint(body)) {
            face.lines.push(line);
        }
    }
    faces
}

/// The hint of a script: the kept lines of each face (a face of a card with several is headed by its name), cut at a
/// line boundary at [`MAX_LINES`] lines or [`MAX_CHARS`] characters with a note. `None` when no line is kept.
pub fn hint_text(script: &str) -> Option<String> {
    let faces = faces_of(script);
    let many = faces.iter().filter(|face| !face.lines.is_empty()).count() > 1;
    let mut text: Vec<String> = Vec::new();
    let (mut lines, mut chars, mut cut) = (0, 0, false);
    'faces: for (at, face) in faces.iter().enumerate() {
        let mut header = many.then(|| format!("Face {} of {}: {}", at + 1, faces.len(), face.name));
        for line in &face.lines {
            let cost = line.chars().count() + 1;
            if lines == MAX_LINES || chars + cost > MAX_CHARS {
                cut = true;
                break 'faces;
            }
            text.extend(header.take());
            text.push((*line).to_owned());
            lines += 1;
            chars += cost;
        }
    }
    if lines == 0 {
        return None;
    }
    if cut {
        text.push(TRUNCATED.to_owned());
    }
    Some(text.join("\n"))
}

/// The scripts of a checkout of Forge, looked up by card name; the folder of a letter is indexed when first needed.
#[derive(Debug)]
pub struct ForgeHints {
    dir: PathBuf,
    letters: Mutex<BTreeMap<char, Arc<BTreeMap<String, PathBuf>>>>,
}

impl ForgeHints {
    /// The scripts under `dir`, which is `forge-gui/res/cardsfolder` or a checkout that has it: the folder must hold a
    /// folder for each first letter.
    pub fn open(dir: &Path) -> Result<Self, CliError> {
        let nested = dir.join("forge-gui/res/cardsfolder");
        let root = if nested.is_dir() {
            nested
        } else {
            dir.to_path_buf()
        };
        let has_letters = fs::read_dir(&root)
            .map_err(|source| CliError::io(&root, source))?
            .flatten()
            .any(|entry| entry.path().is_dir());
        if !has_letters {
            return Err(CliError::Config(format!(
                "--forge-hint {}: no letter folders found; pass `forge-gui/res/cardsfolder` of a checkout of Forge",
                dir.display()
            )));
        }
        Ok(Self {
            dir: root,
            letters: Mutex::default(),
        })
    }

    /// The hint for a card: its script found by its name, else by the name of one of its faces. `None` when the card has
    /// no script or nothing in it to show.
    pub fn hint(&self, card: &PrintedCard) -> Option<String> {
        let faces = card.faces.iter().map(|face| face.name.as_str());
        std::iter::once(card.name.as_str())
            .chain(faces)
            .find_map(|name| self.script(name))
            .and_then(|script| hint_text(&script))
    }

    /// The text of the script that has a face named `name`: the file named by the rule first, else the index of the
    /// folder of the letter.
    fn script(&self, name: &str) -> Option<String> {
        let stem = file_stem(name);
        let letter = stem.chars().next()?;
        let wanted = fold(name);
        let named = self
            .dir
            .join(letter.to_string())
            .join(format!("{stem}.txt"));
        let direct = fs::read(&named)
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .filter(|text| names_of(text).any(|face| fold(face) == wanted));
        if direct.is_some() {
            return direct;
        }
        let path = self.letter(letter).get(&wanted)?.clone();
        let bytes = fs::read(path).ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// The folded name of every face of every script in the folder of `letter`, with the file that holds it; the first
    /// file in name order keeps a name two scripts share.
    fn letter(&self, letter: char) -> Arc<BTreeMap<String, PathBuf>> {
        let mut letters = self.letters.lock().unwrap_or_else(PoisonError::into_inner);
        let index = letters.entry(letter).or_insert_with(|| {
            let mut files: Vec<PathBuf> = fs::read_dir(self.dir.join(letter.to_string()))
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
                .collect();
            files.sort();
            let mut names = BTreeMap::new();
            for file in files {
                let Ok(bytes) = fs::read(&file) else {
                    continue;
                };
                for face in names_of(&String::from_utf8_lossy(&bytes)) {
                    names.entry(fold(face)).or_insert_with(|| file.clone());
                }
            }
            Arc::new(names)
        });
        Arc::clone(index)
    }
}

#[cfg(test)]
mod tests;
