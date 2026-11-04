//! The translation of a card with one gapped line, the oracle and Forge lines of that gap, and the translated lines of
//! other cards that serve as examples.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use mtg_cards::PrintedCard;
use mtg_cards::load::slug;
use mtg_cards::proposal::partial::LedgerLine;
use serde_json::Value;

use super::part::{Kind, Shown};
use crate::forge::script::{Face, Params, Script};

/// The longest example ability shown, in bytes.
const MAX_EXAMPLE: usize = 600;

/// The most script lines a gap shows.
const MAX_FORGE_LINES: usize = 8;

/// The translator's proposal of a card and the ledger of its script lines.
#[derive(Debug, Clone)]
pub struct Translation {
    /// The proposal: `{"faces":[{"abilities":[...]}]}`.
    pub proposal: Value,
    /// One row for each script line.
    pub lines: Vec<LedgerLine>,
}

impl Translation {
    /// Reads `<dir>/<slug>.json` and `<dir>/meta/<slug>.json`; the error says what is missing.
    pub fn load(dir: &Path, name: &str) -> Result<Translation, String> {
        let stem = slug(name);
        let read = |path: PathBuf| {
            let text = fs::read_to_string(&path)
                .map_err(|_| format!("no translation at {}", path.display()))?;
            serde_json::from_str::<Value>(&text)
                .map_err(|error| format!("{}: {error}", path.display()))
        };
        let proposal = read(dir.join(format!("{stem}.json")))?;
        let meta = read(dir.join("meta").join(format!("{stem}.json")))?;
        let lines = meta
            .get("lines")
            .cloned()
            .and_then(|lines| serde_json::from_value(lines).ok())
            .ok_or("the translation has no ledger of lines")?;
        Ok(Translation { proposal, lines })
    }

    /// The one gapped line of the translation: its face, tag, text, key and the place its ability goes. The error names
    /// why the card is not one for line mode.
    pub fn single_gap(&self) -> Result<GapAt, String> {
        let mut seen: BTreeSet<(usize, &str, &str, &str)> = BTreeSet::new();
        let mut first = None;
        for (at, line) in self.lines.iter().enumerate() {
            let Some(key) = &line.gap else { continue };
            if !line.abilities.is_empty() {
                return Err("a line is partly translated".into());
            }
            seen.insert((
                line.face,
                line.tag.as_str(),
                line.text.as_str(),
                key.as_str(),
            ));
            first.get_or_insert(at);
        }
        let Some(first) = first else {
            return Err("no gapped line".into());
        };
        if seen.len() > 1 {
            return Err(format!("{} gapped lines", seen.len()));
        }
        let line = &self.lines[first];
        let before = self.lines[..first]
            .iter()
            .filter(|row| row.face == line.face)
            .flat_map(|row| row.abilities.iter())
            .max()
            .map_or(0, |last| last + 1);
        Ok(GapAt {
            face: line.face,
            tag: line.tag.clone(),
            text: line.text.clone(),
            key: line.gap.clone().unwrap_or_default(),
            at: before,
        })
    }
}

/// Where the gap is in the translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapAt {
    /// The face.
    pub face: usize,
    /// The line's tag letter.
    pub tag: String,
    /// The line's own words (a keyword line: its body).
    pub text: String,
    /// The construct that stopped the line.
    pub key: String,
    /// The index on the face where the ability goes.
    pub at: usize,
}

/// A gapped line with the words the model gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    /// The sentence of the card the line stands for.
    pub oracle: String,
    /// The script line and the named parts it uses.
    pub forge: String,
    /// The construct that stopped the translator.
    pub key: String,
    /// The kind of ability the line is.
    pub kind: Kind,
}

/// The line's own words as the translator's ledger records them.
fn text_of(tag: &str, body: &str) -> String {
    if tag == "K" {
        return body.to_owned();
    }
    let params = Params::parse(body);
    ["SpellDescription", "TriggerDescription", "Description"]
        .iter()
        .find_map(|key| params.get(key))
        .unwrap_or_default()
        .to_owned()
}

/// The words of a text that carry a mechanic: lowercase letters, three or more, without the common ones.
fn words(text: &str) -> BTreeSet<String> {
    const COMMON: &[&str] = &[
        "the", "and", "you", "your", "that", "this", "with", "for", "from", "may", "its", "their",
        "them", "then", "target", "each", "card", "cards", "when", "whenever",
    ];
    text.to_lowercase()
        .split(|c: char| !c.is_alphabetic())
        .filter(|word| word.len() >= 3 && !COMMON.contains(word))
        .map(str::to_owned)
        .collect()
}

/// The script line of the gap and the named parts it reaches, as the script writes them.
fn forge_lines(face: &Face, at: &GapAt) -> Option<(String, Kind)> {
    let bodies = match at.tag.as_str() {
        "K" => &face.keywords,
        "A" => &face.abilities,
        "T" => &face.triggers,
        "S" => &face.statics,
        _ => &face.replacements,
    };
    let body = bodies
        .iter()
        .find(|body| text_of(&at.tag, body) == at.text)?;
    let mut shown = vec![format!("{}:{body}", at.tag)];
    let mut done: BTreeSet<&str> = BTreeSet::new();
    let mut todo = vec![body.as_str()];
    while let Some(text) = todo.pop() {
        let used: BTreeSet<&str> = text
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .collect();
        for (name, value) in &face.svars {
            if name.starts_with("AI") || !used.contains(name.as_str()) || !done.insert(name) {
                continue;
            }
            if shown.len() < MAX_FORGE_LINES {
                shown.push(format!("SVar:{name}:{value}"));
            }
            todo.push(value);
        }
    }
    Some((shown.join("\n"), Kind::of(&at.tag, body)))
}

/// The sentence of the printed face that shares most words with the line; the line itself when the card has no text.
fn oracle_line(card: &PrintedCard, face: usize, text: &str) -> String {
    let mine = words(text);
    let lines = card
        .faces
        .get(face)
        .map_or("", |face| face.oracle_text.as_str())
        .lines();
    lines
        .map(|line| (words(line).intersection(&mine).count(), line))
        .max_by_key(|(shared, _)| *shared)
        .filter(|(shared, _)| *shared > 0)
        .map_or_else(|| text.to_owned(), |(_, line)| line.to_owned())
}

/// The words the model gets for the gap of a card; `None` when the script has no line of that text.
pub fn describe(card: &PrintedCard, script: &str, at: &GapAt) -> Option<Gap> {
    let script = Script::parse(script);
    let (forge, kind) = forge_lines(script.faces.get(at.face)?, at)?;
    Some(Gap {
        oracle: oracle_line(card, at.face, &at.text),
        forge,
        key: at.key.clone(),
        kind,
    })
}

/// The proposal with `ability` inserted on `face` at `at`; every other ability stays as it is.
pub fn merge(proposal: &Value, face: usize, at: usize, ability: Value) -> Option<Value> {
    let mut merged = proposal.clone();
    let list = merged
        .get_mut("faces")?
        .get_mut(face)?
        .get_mut("abilities")?
        .as_array_mut()?;
    if at > list.len() {
        return None;
    }
    list.insert(at, ability);
    Some(merged)
}

/// A translated line of another card that can serve as an example.
#[derive(Debug)]
struct Entry {
    slug: String,
    face: usize,
    ability: usize,
    tag: String,
    text: String,
    words: BTreeSet<String>,
}

/// The translated lines of whole translations (no gap), found by their words.
#[derive(Debug, Default)]
pub struct Pool {
    dir: PathBuf,
    entries: Vec<Entry>,
}

impl Pool {
    /// Reads the ledgers under `<dir>/meta`; a card with a gap gives no line.
    pub fn load(dir: &Path) -> Pool {
        let mut files: Vec<PathBuf> = fs::read_dir(dir.join("meta"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        files.sort();
        let mut entries = Vec::new();
        for file in files {
            let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Some(lines) = fs::read_to_string(&file)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|meta| meta.get("lines").cloned())
                .and_then(|lines| serde_json::from_value::<Vec<LedgerLine>>(lines).ok())
            else {
                continue;
            };
            if lines.iter().any(|line| line.gap.is_some()) {
                continue;
            }
            for line in lines {
                if let [ability] = line.abilities[..] {
                    entries.push(Entry {
                        slug: stem.to_owned(),
                        face: line.face,
                        ability,
                        words: words(&line.text),
                        tag: line.tag,
                        text: line.text,
                    });
                }
            }
        }
        Pool {
            dir: dir.to_path_buf(),
            entries,
        }
    }

    /// The lines of the pool.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the pool holds no line.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Up to `count` examples of `kind` whose words are nearest to `text`, nearest first, each ability once.
    pub fn nearest(&self, kind: Kind, text: &str, count: usize) -> Vec<Shown> {
        let tag = match kind {
            Kind::Triggered => "T",
            Kind::Static => "S",
            Kind::Replacement => "R",
            Kind::Keyword => "K",
            _ => "A",
        };
        let mine = words(text);
        let mut ranked: Vec<(usize, &Entry)> = self
            .entries
            .iter()
            .filter(|entry| entry.tag == tag)
            .map(|entry| (entry.words.intersection(&mine).count(), entry))
            .filter(|(shared, _)| *shared > 0)
            .collect();
        ranked.sort_by(|a, b| {
            (b.0, a.1.text.len(), &a.1.slug).cmp(&(a.0, b.1.text.len(), &b.1.slug))
        });
        let mut shown: Vec<Shown> = Vec::new();
        for (_, entry) in ranked {
            if shown.len() == count {
                break;
            }
            let Some(ability) = self.ability(entry).filter(|ability| kind.has(ability)) else {
                continue;
            };
            let ability = ability.to_string();
            if ability.len() > MAX_EXAMPLE || shown.iter().any(|one| one.ability == ability) {
                continue;
            }
            shown.push(Shown {
                text: entry.text.clone(),
                ability,
            });
        }
        shown
    }

    fn ability(&self, entry: &Entry) -> Option<Value> {
        let text = fs::read_to_string(self.dir.join(format!("{}.json", entry.slug))).ok()?;
        let proposal: Value = serde_json::from_str(&text).ok()?;
        proposal
            .get("faces")?
            .get(entry.face)?
            .get("abilities")?
            .get(entry.ability)
            .cloned()
    }
}
