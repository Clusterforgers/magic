//! Reader of Forge card scripts: the line split, the API and key parse and the `SVar` indirection.
//!
//! A script is read at run time from a local checkout and only its API and key names and counts leave this module.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::CliError;

/// The `Key$ Value` pairs of one line of a script, in written order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Params(Vec<(String, String)>);

impl Params {
    /// Splits a line body at its pipes and each part at its first `$`; a part with no `$` is a key with no value.
    pub fn parse(body: &str) -> Params {
        let pairs = body
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| match part.split_once('$') {
                Some((key, value)) => (key.trim().to_owned(), value.trim().to_owned()),
                None => (part.to_owned(), String::new()),
            })
            .collect();
        Params(pairs)
    }

    /// The value of the first pair with this key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Whether a pair has this key.
    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// The pairs in written order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// The keys that name a variation of the verb, in written order: descriptions, prompts, hints for the computer
    /// player, references to other lines and bookkeeping of remembered objects are left out.
    pub fn meaningful_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        for (key, _) in self.0.iter().skip(1).filter(|(key, _)| !is_noise(key)) {
            let key = canonical_key(key);
            if !keys.iter().any(|seen| seen == key) {
                keys.push(key.to_owned());
            }
        }
        keys
    }
}

/// Whether a key carries no variation of the verb: text for people, hints for the computer player, references that the
/// reader follows, targeting basics, a cost (read as its own parts) and the bookkeeping of remembered objects.
pub fn is_noise(key: &str) -> bool {
    const EXACT: &[&str] = &[
        "Cost",
        "Name",
        "Image",
        "Planeswalker",
        "Ultimate",
        "Secondary",
        "Defined",
        "TgtPrompt",
        "TargetMin",
        "TargetMax",
        "TargetUnique",
        "TgtZone",
        "SubAbility",
        "Execute",
        "Boon",
        "Log",
        "ShowCurrentCard",
        "Origin",
        "Destination",
        "Phase",
        "EffectZone",
        "ActiveZones",
        "ETB",
    ];
    const SUFFIXES: &[&str] = &["Description", "Desc", "Prompt", "Title"];
    const PREFIXES: &[&str] = &[
        "AI",
        "Remember",
        "Forget",
        "Imprint",
        "Clear",
        "Precost",
        "Valid",
        "Affected",
        "Trigger",
        "Activation",
    ];
    EXACT.contains(&key)
        || SUFFIXES.iter().any(|suffix| key.ends_with(suffix))
        || PREFIXES.iter().any(|prefix| key.starts_with(prefix))
}

/// The key as the report names it: the many keys of a condition are `Condition` and those of an "unless" payment `Unless`.
fn canonical_key(key: &str) -> &str {
    const CONDITIONS: &[&str] = &[
        "IsPresent",
        "PresentCompare",
        "PresentZone",
        "PresentPlayer",
        "PresentDefined",
        "CheckSVar",
        "SVarCompare",
    ];
    if key.starts_with("Condition") || CONDITIONS.contains(&key) {
        "Condition"
    } else if key.starts_with("Unless") {
        "Unless"
    } else {
        key
    }
}

/// The family of a part of a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// An ability or sub-ability: the API after `SP$`, `AB$` or `DB$`.
    Effect,
    /// A trigger: the `Mode$`.
    Trigger,
    /// A static ability: the `Mode$`.
    Static,
    /// A replacement effect: the `Event$`.
    Replacement,
    /// A keyword line: the text before the first colon.
    Keyword,
    /// A part of a `Cost$`.
    Cost,
}

impl Kind {
    /// Every family in report order.
    pub const ALL: [Kind; 6] = [
        Kind::Effect,
        Kind::Trigger,
        Kind::Static,
        Kind::Replacement,
        Kind::Keyword,
        Kind::Cost,
    ];

    /// The word the map file and the report use.
    pub const fn word(self) -> &'static str {
        match self {
            Kind::Effect => "effect",
            Kind::Trigger => "trigger",
            Kind::Static => "static",
            Kind::Replacement => "replacement",
            Kind::Keyword => "keyword",
            Kind::Cost => "cost",
        }
    }

    /// The family a word names.
    pub fn from_word(word: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.word() == word)
    }
}

/// One verb a script names: the API, mode, event, keyword or cost part with the keys of its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The family.
    pub kind: Kind,
    /// The API, mode, event, keyword or cost part.
    pub name: String,
    /// The keys of its line (none for a keyword or a cost part).
    pub params: Params,
}

/// One face of a script.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Face {
    /// The `Name:` line.
    pub name: String,
    /// The bodies of the `K:` lines.
    pub keywords: Vec<String>,
    /// The bodies of the `A:` lines.
    pub abilities: Vec<String>,
    /// The bodies of the `T:` lines.
    pub triggers: Vec<String>,
    /// The bodies of the `S:` lines.
    pub statics: Vec<String>,
    /// The bodies of the `R:` lines.
    pub replacements: Vec<String>,
    /// The named parts: `SVar:Name:value`.
    pub svars: BTreeMap<String, String>,
}

/// A script: one face, or several joined by `ALTERNATE` lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Script {
    /// The faces in written order.
    pub faces: Vec<Face>,
}

impl Script {
    /// Reads the text of a script; lines of other tags are skipped.
    pub fn parse(text: &str) -> Script {
        let mut faces = vec![Face::default()];
        for line in text.lines().map(str::trim) {
            if line == "ALTERNATE" {
                faces.push(Face::default());
                continue;
            }
            let Some((tag, body)) = line.split_once(':') else {
                continue;
            };
            let Some(face) = faces.last_mut() else {
                continue;
            };
            let body = body.trim();
            match tag {
                "Name" => body.clone_into(&mut face.name),
                "K" => face.keywords.push(body.to_owned()),
                "A" => face.abilities.push(body.to_owned()),
                "T" => face.triggers.push(body.to_owned()),
                "S" => face.statics.push(body.to_owned()),
                "R" => face.replacements.push(body.to_owned()),
                "SVar" => {
                    if let Some((name, value)) = body.split_once(':') {
                        face.svars
                            .insert(name.trim().to_owned(), value.trim().to_owned());
                    }
                }
                _ => {}
            }
        }
        faces.retain(|face| !face.name.is_empty());
        Script { faces }
    }

    /// The name of every face.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.faces.iter().map(|face| face.name.as_str())
    }

    /// Everything the script names, with every part reached through an `SVar` reference.
    pub fn items(&self) -> Vec<Item> {
        self.faces.iter().flat_map(Face::items).collect()
    }
}

impl Face {
    /// The items of the face: its lines, the parts they name (`SubAbility$`, `Execute$`, `Choices$` and the like), and the
    /// ability, trigger, static and replacement parts that no line names.
    pub fn items(&self) -> Vec<Item> {
        let mut walk = Walk {
            svars: &self.svars,
            seen: BTreeSet::new(),
            items: Vec::new(),
        };
        for body in &self.abilities {
            walk.line(Kind::Effect, Params::parse(body));
        }
        for body in &self.triggers {
            walk.line(Kind::Trigger, Params::parse(body));
        }
        for body in &self.statics {
            walk.line(Kind::Static, Params::parse(body));
        }
        for body in &self.replacements {
            walk.line(Kind::Replacement, Params::parse(body));
        }
        for body in &self.keywords {
            walk.keyword(body);
        }
        for (name, value) in &self.svars {
            if !walk.seen.contains(name.as_str()) {
                walk.svar(name, value, None);
            }
        }
        walk.items
    }
}

/// The walk over one face.
struct Walk<'a> {
    svars: &'a BTreeMap<String, String>,
    seen: BTreeSet<&'a str>,
    items: Vec<Item>,
}

impl<'a> Walk<'a> {
    /// Records a line and follows the parts its values name.
    fn line(&mut self, kind: Kind, params: Params) {
        let name = match kind {
            Kind::Effect => ["SP", "AB", "DB"]
                .into_iter()
                .find_map(|head| params.get(head)),
            Kind::Trigger | Kind::Static => params.get("Mode"),
            Kind::Replacement => params.get("Event"),
            Kind::Keyword | Kind::Cost => None,
        };
        let Some(name) = name.map(str::to_owned) else {
            return;
        };
        // A spell repeats its mana cost, which the printed side already holds.
        let spell = params.get("SP").is_some();
        let costs = match params.get("Cost") {
            Some(cost) if kind == Kind::Effect => cost_parts(cost)
                .into_iter()
                .filter(|part| !(spell && part == "Mana"))
                .collect(),
            _ => Vec::new(),
        };
        let names: Vec<(Option<Kind>, String)> = params
            .iter()
            .filter(|(key, _)| !key.ends_with("Description") && !key.ends_with("Desc"))
            .flat_map(|(key, value)| {
                references(value)
                    .into_iter()
                    .map(move |token| (hint(key), token))
            })
            .collect();
        self.items.push(Item { kind, name, params });
        for part in costs {
            self.items.push(Item {
                kind: Kind::Cost,
                name: part,
                params: Params::default(),
            });
        }
        for (hint, token) in names {
            self.reference(&token, hint);
        }
    }

    /// Records a keyword line and follows the parts it names.
    fn keyword(&mut self, body: &str) {
        let name = body.split(':').next().unwrap_or(body).trim().to_owned();
        self.items.push(Item {
            kind: Kind::Keyword,
            name,
            params: Params::default(),
        });
        for token in body.split(':').skip(1).flat_map(references) {
            self.reference(&token, None);
        }
    }

    /// Follows a name to the part it stands for, once.
    fn reference(&mut self, token: &str, hint: Option<Kind>) {
        let svars = self.svars;
        let Some((name, value)) = svars.get_key_value(token) else {
            return;
        };
        if self.seen.insert(name.as_str()) {
            self.svar(name, value, hint);
        }
    }

    /// Reads the value of a named part by its shape: a part is an ability, a trigger, a static ability or a replacement
    /// effect; an amount, a count and a hint for the computer player are not items.
    fn svar(&mut self, name: &'a str, value: &str, hint: Option<Kind>) {
        self.seen.insert(name);
        let params = Params::parse(value);
        let Some((first, _)) = params.iter().next() else {
            return;
        };
        let kind = match first {
            "SP" | "AB" | "DB" => Kind::Effect,
            "Event" => Kind::Replacement,
            "Mode" => hint
                .filter(|kind| matches!(kind, Kind::Trigger | Kind::Static))
                .unwrap_or(if params.has("Execute") {
                    Kind::Trigger
                } else {
                    Kind::Static
                }),
            _ => return,
        };
        self.line(kind, params);
    }
}

/// The kind of part a key names: `Triggers$`, `AddStaticAbility$`, `ReplacementEffects$`.
fn hint(key: &str) -> Option<Kind> {
    let lower = key.to_lowercase();
    if lower.contains("trigger") {
        Some(Kind::Trigger)
    } else if lower.contains("static") {
        Some(Kind::Static)
    } else if lower.contains("replacement") {
        Some(Kind::Replacement)
    } else {
        None
    }
}

/// The names a value may stand for: its parts between commas and ampersands, without a sign.
fn references(value: &str) -> Vec<String> {
    value
        .split([',', '&'])
        .map(|token| token.trim().trim_start_matches(['+', '-']))
        .filter(|token| !token.is_empty() && !token.contains(' '))
        .map(str::to_owned)
        .collect()
}

/// The parts of a `Cost$`, each as its name: `T`, `Sac`, `PayLife`, or `Mana` for the mana symbols.
pub fn cost_parts(cost: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0u32;
    for c in cost.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if c.is_whitespace() && depth == 0 {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    parts.push(current);
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(|part| cost_name(&part))
        .collect()
}

/// The name of one cost part: the word before its `<`, or `Mana` for a number or mana symbols. A part that names the
/// source (`Sac<1/CARDNAME>`) ends in `Me`, and a part on loyalty counters is `Loyalty`.
fn cost_name(part: &str) -> String {
    if part
        .chars()
        .all(|c| c.is_ascii_digit() || "XWUBRGCSP/".contains(c))
    {
        return "Mana".to_owned();
    }
    let Some((word, rest)) = part.split_once('<') else {
        return part.to_owned();
    };
    let what = rest.trim_end_matches('>').split('/').nth(1).unwrap_or("");
    if what.eq_ignore_ascii_case("loyalty") {
        "Loyalty".to_owned()
    } else if what == "CARDNAME" || what.starts_with("Card.Self") {
        format!("{}Me", word.trim_end_matches("Me"))
    } else {
        word.to_owned()
    }
}

/// Lowercase with the accents and typographic quotes of Latin text folded, for matching card names.
pub fn fold(name: &str) -> String {
    name.chars()
        .flat_map(|c| {
            let folded: &str = match c {
                '\u{2018}' | '\u{2019}' => "'",
                '\u{201c}' | '\u{201d}' => "\"",
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => {
                    "a"
                }
                'ç' | 'Ç' => "c",
                'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => "e",
                'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => "i",
                'ñ' | 'Ñ' => "n",
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => "o",
                'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => "u",
                'ý' | 'ÿ' | 'Ý' => "y",
                'æ' | 'Æ' => "ae",
                'œ' | 'Œ' => "oe",
                'ß' => "ss",
                _ => return c.to_lowercase().collect::<Vec<_>>(),
            };
            folded.chars().collect()
        })
        .collect()
}

#[cfg(test)]
mod tests;

/// Reads every script under a checkout of Forge: the directory given, or its `forge-gui/res/cardsfolder` when that exists.
/// The directory of rebalanced cards is skipped.
pub fn read_checkout(dir: &Path) -> Result<Vec<Script>, CliError> {
    let nested = dir.join("forge-gui/res/cardsfolder");
    let root = if nested.is_dir() {
        nested
    } else {
        dir.to_path_buf()
    };
    let mut files = Vec::new();
    collect_files(&root, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(CliError::Config(format!(
            "{}: no Forge card scripts (`*.txt`) found; pass a checkout of Forge with `forge-gui/res/cardsfolder`",
            dir.display()
        )));
    }
    let mut scripts = Vec::with_capacity(files.len());
    for file in files {
        let bytes = fs::read(&file).map_err(|source| CliError::io(&file, source))?;
        let script = Script::parse(&String::from_utf8_lossy(&bytes));
        if !script.faces.is_empty() {
            scripts.push(script);
        }
    }
    Ok(scripts)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), CliError> {
    let entries = fs::read_dir(dir).map_err(|source| CliError::io(dir, source))?;
    for entry in entries {
        let path = entry.map_err(|source| CliError::io(dir, source))?.path();
        if path.is_dir() {
            if path.file_name().is_none_or(|name| name != "rebalanced") {
                collect_files(&path, files)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "txt") {
            files.push(path);
        }
    }
    Ok(())
}
