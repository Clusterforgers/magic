//! The kinds of ability a gapped line can be, the JSON schema and short shape of each, and the small prompt of one line.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use mtg_cards::reference::w3a_card_schema;
use serde_json::{Map, Value, json};

use super::pool::Gap;
use crate::propose::model::{Message, Request};
use crate::propose::prompt::CARD_MARKER;

/// The most tokens an answer of one line may have, a reasoning model's thinking included.
pub const LINE_ANSWER_TOKENS: u32 = 4096;

/// What a gapped line becomes in the DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A spell effect (`SP$`).
    Spell,
    /// An activated ability (`AB$`).
    Activated,
    /// A planeswalker's loyalty ability.
    Loyalty,
    /// A triggered ability.
    Triggered,
    /// A static ability or another continuous rule.
    Static,
    /// A replacement effect.
    Replacement,
    /// A keyword line.
    Keyword,
}

impl Kind {
    /// The kind of a script line from its tag and body.
    pub fn of(tag: &str, body: &str) -> Kind {
        let body = body.trim_start();
        match tag {
            "T" => Kind::Triggered,
            "S" => Kind::Static,
            "R" => Kind::Replacement,
            "K" => Kind::Keyword,
            _ if body.contains("Planeswalker$ True") => Kind::Loyalty,
            _ if body.starts_with("AB$") => Kind::Activated,
            _ => Kind::Spell,
        }
    }

    /// The names of the `Ability` variants of this kind.
    pub fn variants(self) -> &'static [&'static str] {
        match self {
            Kind::Spell => &["Spell"],
            Kind::Activated => &["Activated"],
            Kind::Loyalty => &["Loyalty"],
            Kind::Triggered => &["Triggered"],
            Kind::Static => &[
                "Static",
                "Rule",
                "CostModifier",
                "AdditionalCost",
                "FreeCast",
            ],
            Kind::Replacement => &["Replacement"],
            Kind::Keyword => &["Keyword"],
        }
    }

    /// The kind as a word.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Spell => "spell effect",
            Kind::Activated => "activated ability",
            Kind::Loyalty => "loyalty ability",
            Kind::Triggered => "triggered ability",
            Kind::Static => "static ability",
            Kind::Replacement => "replacement effect",
            Kind::Keyword => "keyword",
        }
    }

    /// Whether an ability of the DSL, as JSON, is of this kind.
    pub fn has(self, ability: &Value) -> bool {
        ability.as_object().is_some_and(|map| {
            map.keys()
                .any(|key| self.variants().contains(&key.as_str()))
        })
    }
}

/// The variant objects of `Ability` that belong to `kind`, with the definitions they reach.
fn pick(kind: Kind) -> (Vec<Value>, Map<String, Value>) {
    let schema = w3a_card_schema();
    let defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let options = defs
        .get("Ability")
        .and_then(|ability| ability.get("anyOf"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let chosen = options
        .into_iter()
        .filter(|option| {
            option
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|props| {
                    props
                        .keys()
                        .any(|key| kind.variants().contains(&key.as_str()))
                })
        })
        .collect();
    (chosen, defs)
}

/// The names a schema value refers to.
fn refs(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                match (key.as_str(), inner.as_str()) {
                    ("$ref", Some(target)) => {
                        out.insert(target.trim_start_matches("#/$defs/").to_owned());
                    }
                    _ => refs(inner, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| refs(item, out)),
        _ => {}
    }
}

/// The JSON schema of one ability of this kind, derived from the DSL types: the variant (or variants) of `Ability` as the
/// root, with only the definitions it reaches. It uses the keywords the grammar converters of Ollama and llama.cpp read.
pub fn schema(kind: Kind) -> Value {
    let (mut chosen, defs) = pick(kind);
    let mut root = if chosen.len() == 1 {
        chosen.remove(0)
    } else {
        json!({"anyOf": chosen})
    };
    let mut wanted = BTreeSet::new();
    refs(&root, &mut wanted);
    let mut kept = Map::new();
    let mut todo: Vec<String> = wanted.into_iter().collect();
    while let Some(name) = todo.pop() {
        if kept.contains_key(&name) {
            continue;
        }
        let Some(def) = defs.get(&name) else { continue };
        let mut inner = BTreeSet::new();
        refs(def, &mut inner);
        todo.extend(inner);
        kept.insert(name, def.clone());
    }
    if let Value::Object(map) = &mut root {
        map.insert("$defs".into(), Value::Object(kept));
    }
    root
}

/// A short name for the type a schema value stands for.
fn type_name(value: &Value) -> String {
    if let Some(target) = value.get("$ref").and_then(Value::as_str) {
        return target.trim_start_matches("#/$defs/").to_owned();
    }
    if let Some(options) = value.get("anyOf").and_then(Value::as_array) {
        let names: Vec<String> = options.iter().map(type_name).collect();
        return names.join(" | ");
    }
    match value.get("type").and_then(Value::as_str) {
        Some("array") => format!("[{}]", value.get("items").map_or("?".into(), type_name)),
        Some("object") => "{..}".into(),
        Some(other) => other.to_owned(),
        None => "?".into(),
    }
}

/// The shape of the variants of `kind`, one line each: `Triggered {trigger: Trigger, effects: [Effect], condition?: Condition}`.
pub fn shape(kind: Kind) -> String {
    let (chosen, _) = pick(kind);
    let mut text = String::new();
    for option in &chosen {
        let Some((name, payload)) = option
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|props| props.iter().next())
        else {
            continue;
        };
        let fields = payload.get("properties").and_then(Value::as_object);
        let required: Vec<&str> = payload
            .get("required")
            .and_then(Value::as_array)
            .map(|all| all.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        match fields {
            Some(fields) => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(field, ty)| {
                        let mark = if required.contains(&field.as_str()) {
                            ""
                        } else {
                            "?"
                        };
                        format!("{field}{mark}: {}", type_name(ty))
                    })
                    .collect();
                let _ = writeln!(text, "{{\"{name}\": {{{}}}}}", parts.join(", "));
            }
            None => {
                let _ = writeln!(text, "{{\"{name}\": {}}}", type_name(payload));
            }
        }
    }
    text.trim_end().to_owned()
}

/// The system prompt of a line request.
pub const LINE_SYSTEM: &str = "\
You write one ability of a Magic: The Gathering card as JSON for a rules engine. The rest of the card is already written; \
you write only the missing ability, from the oracle line and the Forge script lines of it. Reply with one JSON object that \
is that single ability and nothing else, in the shape given. Follow the examples' spelling of every variant and field; \
never invent one. The oracle line is the only ground truth; the Forge lines only show what to look for.";

/// An example of a line: the words and the ability written for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The words of the line.
    pub text: String,
    /// The ability as compact JSON.
    pub ability: String,
}

/// The request for one gapped line: its oracle line, the Forge lines, the gap, the shape and the examples.
pub fn request(model: &str, name: &str, gap: &Gap, kind: Kind, examples: &[Shown]) -> Request {
    let mut text = format!("{CARD_MARKER}{name}\n");
    let _ = writeln!(text, "Oracle line to write: {}", gap.oracle);
    let _ = writeln!(text, "Forge script lines of it:\n{}", gap.forge);
    let _ = writeln!(text, "Why the translator stopped: {}", gap.key);
    let _ = writeln!(
        text,
        "Write one {} of this shape:\n{}",
        kind.word(),
        shape(kind)
    );
    if !examples.is_empty() {
        text.push_str("Examples of similar lines, as the rules write them:\n");
        for shown in examples {
            let _ = writeln!(text, "- {} => {}", shown.text, shown.ability);
        }
    }
    text.push_str("Reply with the JSON of the one ability only.");
    Request {
        model: model.to_owned(),
        system: LINE_SYSTEM.to_owned(),
        messages: vec![Message::user(text)],
        max_tokens: LINE_ANSWER_TOKENS,
    }
}

/// The message of the repair round: what is wrong with the answer.
pub fn repair_message(problem: &str) -> String {
    format!("That does not work: {problem}\nReply with the corrected JSON of the one ability only.")
}
