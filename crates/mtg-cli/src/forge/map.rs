//! The map from the Forge APIs, trigger and static modes, replacement events, keywords and cost parts to the verbs of
//! our DSL, read from `map.txt`: names of both sides only.
//!
//! Lines of the file (`#` starts a comment):
//!
//! - `effect Draw => Effect.Draw`: a Forge item and the verbs of ours that stand for it (any one is enough). Conditions
//!   after the name (`Destination=Exile|Hand`, or a bare key) make a rule more specific; the most specific rule that
//!   holds wins.
//! - `-` as the target: the DSL has no verb for it. `~`: bookkeeping that needs no verb.
//! - `also effect CopyPermanent NonLegendary => Mod.RemoveSupertype`: a key on a Forge line that asks for a further
//!   verb (`*` for any name of the family). Conditions after the key (`also static Continuous AddAbility
//!   AffectedDefined=Self => ...`) make the rule more specific: for one key the rules with the most conditions that
//!   hold decide.
//! - `variant effect ChangeZone Destination`: a key whose value tells apart the uses of the verb in the report.
//!
//! A keyword with no rule stands for the keyword of the same name.

use std::sync::OnceLock;

use super::script::{Item, Kind, Params};
use crate::error::CliError;

const STANDARD: &str = include_str!("map.txt");

/// A name reduced to lowercase letters and digits, so that `First Strike` and `FirstStrike` compare equal.
pub fn norm(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
        .flat_map(char::to_lowercase)
        .collect()
}

/// A condition on one key of a line: the key is there, with one of the values when some are listed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cond {
    key: String,
    values: Vec<String>,
}

impl Cond {
    fn parse(token: &str) -> Cond {
        match token.split_once('=') {
            Some((key, values)) => Cond {
                key: key.to_owned(),
                values: values.split('|').map(str::to_owned).collect(),
            },
            None => Cond {
                key: token.to_owned(),
                values: Vec::new(),
            },
        }
    }

    fn holds(&self, params: &Params) -> bool {
        params.get(&self.key).is_some_and(|value| {
            self.values.is_empty() || self.values.iter().any(|wanted| wanted == value)
        })
    }
}

/// What the map says of a Forge item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    /// The verbs of ours that stand for it; one of them is enough.
    Verbs(Vec<String>),
    /// The DSL has no verb for it.
    NoVerb,
    /// Bookkeeping: nothing is expected.
    Skip,
    /// The map does not name it.
    Unmapped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Verbs(Vec<String>),
    NoVerb,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    kind: Kind,
    name: String,
    when: Vec<Cond>,
    target: Target,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Extra {
    kind: Kind,
    name: String,
    when: Cond,
    and: Vec<Cond>,
    verbs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Variant {
    kind: Kind,
    name: String,
    key: String,
}

/// A key of a Forge line that asks for a verb the line's own verb does not give.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// The key.
    pub key: String,
    /// The verbs of ours that stand for it; one is enough.
    pub verbs: Vec<String>,
}

/// The map: rules, the keys that ask for more, and the keys that tell uses apart.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Map {
    rules: Vec<Rule>,
    extras: Vec<Extra>,
    variants: Vec<Variant>,
}

impl Map {
    /// The map embedded from `map.txt`.
    pub fn standard() -> Result<&'static Map, CliError> {
        static MAP: OnceLock<Result<Map, String>> = OnceLock::new();
        MAP.get_or_init(|| Map::parse(STANDARD))
            .as_ref()
            .map_err(|error| CliError::Config(format!("the embedded Forge map: {error}")))
    }

    /// Reads a map; an error names the line.
    pub fn parse(text: &str) -> Result<Map, String> {
        let mut map = Map::default();
        for (index, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            map.add(line)
                .map_err(|error| format!("line {}: {error}", index + 1))?;
        }
        Ok(map)
    }

    fn add(&mut self, line: &str) -> Result<(), String> {
        let (left, right) = match line.split_once("=>") {
            Some((left, right)) => (left, Some(right)),
            None => (line, None),
        };
        let mut words = left.split_whitespace();
        let first = words.next().ok_or("empty line")?;
        let (extra, variant) = (first == "also", first == "variant");
        let kind_word = if extra || variant {
            words.next()
        } else {
            Some(first)
        };
        let kind = kind_word
            .and_then(Kind::from_word)
            .ok_or_else(|| format!("`{}` is not a family", kind_word.unwrap_or("")))?;
        let name = match words.next().ok_or("no name")? {
            "*" => "*".to_owned(),
            name => norm(name),
        };
        let rest: Vec<&str> = words.collect();
        let verbs = |right: Option<&str>| -> Result<Vec<String>, String> {
            let verbs: Vec<String> = right
                .ok_or("no `=>`")?
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            if verbs.is_empty() {
                return Err("no verbs after `=>`".into());
            }
            Ok(verbs)
        };
        if variant {
            let key = rest.first().ok_or("no key")?;
            self.variants.push(Variant {
                kind,
                name,
                key: (*key).to_owned(),
            });
        } else if extra {
            let (key, and) = rest.split_first().ok_or("no key")?;
            self.extras.push(Extra {
                kind,
                name,
                when: Cond::parse(key),
                and: and.iter().map(|cond| Cond::parse(cond)).collect(),
                verbs: verbs(right)?,
            });
        } else {
            let verbs = verbs(right)?;
            let target = match verbs.as_slice() {
                [only] if only == "-" => Target::NoVerb,
                [only] if only == "~" => Target::Skip,
                _ => Target::Verbs(verbs),
            };
            self.rules.push(Rule {
                kind,
                name,
                when: rest.into_iter().map(Cond::parse).collect(),
                target,
            });
        }
        Ok(())
    }

    /// What the map says of an item: the most specific rule that holds, else (for a keyword) the keyword of the same
    /// name, else nothing.
    pub fn lookup(&self, item: &Item) -> Lookup {
        let name = norm(&item.name);
        let mut best: Option<&Rule> = None;
        for rule in &self.rules {
            if rule.kind == item.kind
                && rule.name == name
                && rule.when.iter().all(|cond| cond.holds(&item.params))
                && best.is_none_or(|held| rule.when.len() > held.when.len())
            {
                best = Some(rule);
            }
        }
        match best {
            Some(Rule {
                target: Target::Verbs(verbs),
                ..
            }) => Lookup::Verbs(verbs.clone()),
            Some(Rule {
                target: Target::NoVerb,
                ..
            }) => Lookup::NoVerb,
            Some(Rule {
                target: Target::Skip,
                ..
            }) => Lookup::Skip,
            None if item.kind == Kind::Keyword => Lookup::Verbs(keyword_verbs(&item.name)),
            None => Lookup::Unmapped,
        }
    }

    /// The keys of an item that ask for verbs of their own; for one key the rules with the most conditions that hold decide.
    pub fn wanted(&self, item: &Item) -> Vec<Wanted> {
        let name = norm(&item.name);
        let held: Vec<&Extra> = self
            .extras
            .iter()
            .filter(|extra| {
                extra.kind == item.kind
                    && (extra.name == name || extra.name == "*")
                    && extra.when.holds(&item.params)
                    && extra.and.iter().all(|cond| cond.holds(&item.params))
            })
            .collect();
        held.iter()
            .filter(|extra| {
                held.iter()
                    .filter(|other| other.when.key == extra.when.key)
                    .all(|other| other.and.len() <= extra.and.len())
            })
            .map(|extra| Wanted {
                key: extra.when.key.clone(),
                verbs: extra.verbs.clone(),
            })
            .collect()
    }

    /// The keys of an item that tell its uses apart, each with its value (`Destination=Exile`).
    pub fn variants(&self, item: &Item) -> Vec<String> {
        let name = norm(&item.name);
        self.variants
            .iter()
            .filter(|variant| variant.kind == item.kind && variant.name == name)
            .filter_map(|variant| {
                item.params
                    .get(&variant.key)
                    .map(|value| format!("{}={value}", variant.key))
            })
            .collect()
    }

    /// Every verb the map names, without the markers for none and bookkeeping.
    pub fn verbs(&self) -> impl Iterator<Item = &str> {
        let rules = self.rules.iter().filter_map(|rule| match &rule.target {
            Target::Verbs(verbs) => Some(verbs),
            Target::NoVerb | Target::Skip => None,
        });
        rules
            .chain(self.extras.iter().map(|extra| &extra.verbs))
            .flatten()
            .map(String::as_str)
    }

    /// How many rules name this family.
    pub fn rules_of(&self, kind: Kind) -> usize {
        self.rules.iter().filter(|rule| rule.kind == kind).count()
    }
}

#[cfg(test)]
mod tests;

/// The verbs a keyword line stands for: the keyword written in one word, and the keyword of its first word
/// (`Enchant creature` is `Enchant`).
fn keyword_verbs(name: &str) -> Vec<String> {
    let camel = |text: &str| -> String {
        text.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_default()
            })
            .collect()
    };
    let mut verbs = vec![format!("Keyword.{}", camel(name))];
    if let Some(first) = name.split_whitespace().next() {
        let first = format!("Keyword.{}", camel(first));
        if !verbs.contains(&first) {
            verbs.push(first);
        }
    }
    verbs
}
