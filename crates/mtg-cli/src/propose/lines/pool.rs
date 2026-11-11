//! The translation of a card with one or two gapped lines, the oracle and Forge lines of each gap, and the translated lines
//! of other cards that serve as examples, found by the shape of their Forge line first and their words second.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mtg_cards::PrintedCard;
use mtg_cards::load::slug;
use mtg_cards::proposal::partial::LedgerLine;
use serde_json::Value;

use super::part::{Kind, Shown};
use crate::forge::script::{Face, Params, Script};
use crate::propose::forge_hint::ForgeHints;

/// The most gapped lines a card may have for line mode.
pub const MAX_GAPS: usize = 2;

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

    /// The gapped lines of the translation (at most [`MAX_GAPS`]) in script order, each with its face, tag, text, key and
    /// the place its ability goes in the translation as it is. The error names why the card is not one for line mode.
    pub fn gaps(&self) -> Result<Vec<GapAt>, String> {
        let mut seen: BTreeSet<(usize, &str, &str, &str)> = BTreeSet::new();
        let mut firsts = Vec::new();
        for (at, line) in self.lines.iter().enumerate() {
            let Some(key) = &line.gap else { continue };
            if !line.abilities.is_empty() {
                return Err("a line is partly translated".into());
            }
            let fresh = seen.insert((
                line.face,
                line.tag.as_str(),
                line.text.as_str(),
                key.as_str(),
            ));
            if fresh {
                firsts.push(at);
            }
        }
        match firsts.len() {
            0 => return Err("no gapped line".into()),
            count if count > MAX_GAPS => return Err(format!("{count} gapped lines")),
            _ => {}
        }
        Ok(firsts
            .into_iter()
            .map(|first| {
                let line = &self.lines[first];
                let before = self.lines[..first]
                    .iter()
                    .filter(|row| row.face == line.face)
                    .flat_map(|row| row.abilities.iter())
                    .max()
                    .map_or(0, |last| last + 1);
                GapAt {
                    face: line.face,
                    tag: line.tag.clone(),
                    text: line.text.clone(),
                    key: line.gap.clone().unwrap_or_default(),
                    at: before,
                }
            })
            .collect())
    }
}

/// Where a gap is in the translation.
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
    /// The index on the face where the ability goes, in the translation as it is.
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

/// The words of a line with the words of the card's own name masked out, so that a name does not count as a mechanic.
fn masked(text: &str, name: &str) -> BTreeSet<String> {
    let own = words(name);
    words(text)
        .into_iter()
        .filter(|word| !own.contains(word))
        .collect()
}

/// What a script line does: the verbs it and its named parts use (`api:Dig`, `mode:Attacks`) and the keys that vary them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shape {
    verbs: BTreeSet<String>,
    keys: BTreeSet<String>,
}

impl Shape {
    /// The shape of script lines as a gap shows them (`T:...`, `SVar:Name:...`, one a line).
    pub fn of(forge: &str) -> Shape {
        let mut shape = Shape::default();
        for line in forge.lines() {
            let body = match line.split_once(':') {
                Some(("SVar", rest)) => rest.split_once(':').map_or(rest, |(_, body)| body),
                Some(("K", rest)) => {
                    let word = rest.split([':', ' ']).next().unwrap_or(rest);
                    shape.verbs.insert(format!("kw:{word}"));
                    continue;
                }
                Some((_, rest)) => rest,
                None => line,
            };
            let params = Params::parse(body);
            if let Some((key, value)) = params.iter().next() {
                let key = match key {
                    "SP" | "AB" | "DB" => "api",
                    other => other,
                };
                shape.verbs.insert(format!("{key}:{}", value.trim()));
            }
            shape.keys.extend(params.meaningful_keys());
        }
        shape
    }

    fn shared(&self, other: &Shape) -> (usize, usize) {
        (
            self.verbs.intersection(&other.verbs).count(),
            self.keys.intersection(&other.keys).count(),
        )
    }
}

/// The body of a script line by its tag and the words the ledger records for it.
fn body_of<'f>(face: &'f Face, tag: &str, text: &str) -> Option<&'f String> {
    let bodies = match tag {
        "K" => &face.keywords,
        "A" => &face.abilities,
        "T" => &face.triggers,
        "S" => &face.statics,
        _ => &face.replacements,
    };
    bodies.iter().find(|body| text_of(tag, body) == text)
}

/// The script line of the gap and the named parts it reaches, as the script writes them.
fn forge_lines(face: &Face, tag: &str, text: &str) -> Option<(String, Kind)> {
    let body = body_of(face, tag, text)?;
    let mut shown = vec![format!("{tag}:{body}")];
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
    Some((shown.join("\n"), Kind::of(tag, body)))
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

/// The words the model gets for a gap of a card; `None` when the script has no line of that text.
pub fn describe(card: &PrintedCard, script: &str, at: &GapAt) -> Option<Gap> {
    let script = Script::parse(script);
    let (forge, kind) = forge_lines(script.faces.get(at.face)?, &at.tag, &at.text)?;
    Some(Gap {
        oracle: oracle_line(card, at.face, &at.text),
        forge,
        key: at.key.clone(),
        kind,
    })
}

/// The proposal with `ability` inserted on `face` at `at`; every other ability stays as it is.
pub fn merge(proposal: &Value, face: usize, at: usize, ability: Value) -> Option<Value> {
    merge_all(proposal, vec![(face, at, ability)])
}

/// The proposal with each `(face, at, ability)` inserted, every place counted in the proposal as it is; abilities for the
/// same place keep their order.
pub fn merge_all(proposal: &Value, parts: Vec<(usize, usize, Value)>) -> Option<Value> {
    let mut merged = proposal.clone();
    let mut parts: Vec<(usize, (usize, usize, Value))> = parts.into_iter().enumerate().collect();
    // From the last place to the first, so that an insertion never moves a place still to come.
    parts.sort_by_key(|part| std::cmp::Reverse((part.1.0, part.1.1, part.0)));
    for (_, (face, at, ability)) in parts {
        let list = merged
            .get_mut("faces")?
            .get_mut(face)?
            .get_mut("abilities")?
            .as_array_mut()?;
        if at > list.len() {
            return None;
        }
        list.insert(at, ability);
    }
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
    shape: Shape,
}

/// The translated lines of whole translations (no gap), found by the shape of their script line and by their words.
#[derive(Debug, Default)]
pub struct Pool {
    dir: PathBuf,
    entries: Vec<Entry>,
}

impl Pool {
    /// Reads the ledgers under `<dir>/meta`; a card with a gap gives no line. No line has a script shape.
    pub fn load(dir: &Path) -> Pool {
        Pool::load_with(dir, None, &[])
    }

    /// [`Pool::load`] that also reads the shape of each line from the card's script in `forge`, the cards found by slug
    /// among `cards`.
    pub fn load_with(dir: &Path, forge: Option<&ForgeHints>, cards: &[PrintedCard]) -> Pool {
        let by_slug: BTreeMap<String, &PrintedCard> =
            cards.iter().map(|card| (slug(&card.name), card)).collect();
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
            let card = by_slug.get(stem).copied();
            let script = forge
                .zip(card)
                .and_then(|(forge, card)| forge.script_of(card))
                .map(|text| Script::parse(&text));
            let name = card.map_or("", |card| card.name.as_str());
            for line in lines {
                let [ability] = line.abilities[..] else {
                    continue;
                };
                let shape = script
                    .as_ref()
                    .and_then(|script| script.faces.get(line.face))
                    .and_then(|face| forge_lines(face, &line.tag, &line.text))
                    .map(|(shown, _)| Shape::of(&shown))
                    .unwrap_or_default();
                entries.push(Entry {
                    slug: stem.to_owned(),
                    face: line.face,
                    ability,
                    words: masked(&line.text, name),
                    shape,
                    tag: line.tag,
                    text: line.text,
                });
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

    /// Up to `count` examples of `kind` nearest to a gap: the most verbs of its script shape shared first, then the most
    /// varying keys, then the most words of its text (the card's `name` masked out); each ability once.
    pub fn nearest(
        &self,
        kind: Kind,
        shape: &Shape,
        text: &str,
        name: &str,
        count: usize,
    ) -> Vec<Shown> {
        let tag = match kind {
            Kind::Triggered => "T",
            Kind::Static => "S",
            Kind::Replacement => "R",
            Kind::Keyword => "K",
            _ => "A",
        };
        let mine = masked(text, name);
        let mut ranked: Vec<((usize, usize, usize), &Entry)> = self
            .entries
            .iter()
            .filter(|entry| entry.tag == tag)
            .map(|entry| {
                let (verbs, keys) = shape.shared(&entry.shape);
                (
                    (verbs, keys, entry.words.intersection(&mine).count()),
                    entry,
                )
            })
            .filter(|((verbs, _, shared), _)| *verbs > 0 || *shared > 0)
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
