//! The kinds of ability a gapped line can be, the JSON schema and short shape of each, the small prompt of one line and
//! the reading of its answer.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use mtg_cards::ability::Ability;
use mtg_cards::reference::w3a_card_schema;
use serde_json::{Map, Value, json};

use super::pool::Gap;
use crate::propose::model::{Message, Request};
use crate::propose::prompt::{CARD_MARKER, extract_json};

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

/// The instructions of a line request; the shape of the kind follows them in the system prompt.
pub const LINE_SYSTEM: &str = "\
You write one ability of a Magic: The Gathering card as JSON for a rules engine. The rest of the card is already written; \
you write only the missing ability, from the oracle line and the Forge script lines of it. Reply with one JSON object that \
is that single ability and nothing else, in the shape given. Follow the examples' spelling of every variant and field; \
never invent one. The oracle line is the only ground truth; the Forge lines only show what to look for. \
Never approximate: when the rules cannot say the whole line exactly, reply {\"missing\": \"<the engine piece the line \
needs>\"} instead, and nothing else.";

/// The system prompt of a line request for an ability of `kind`: the instructions and the shape of the kind. It is the
/// same for every line of the kind, so the API caches it.
pub fn system(kind: Kind) -> String {
    format!(
        "{LINE_SYSTEM}\n\nThe shape of a {} (`?` marks a field that may be left out):\n{}",
        kind.word(),
        shape(kind)
    )
}

/// An example of a line: the words and the ability written for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The words of the line.
    pub text: String,
    /// The ability as compact JSON.
    pub ability: String,
}

/// The most bytes of the DSL reference a line's prompt holds (about 5000 tokens).
pub const REFERENCE_BUDGET: usize = 15_000;

/// The words that pick the parts of the reference a line needs: the oracle line's words, the identifiers of its Forge
/// lines (whole and split at capitals, `ChangeZone` as `changezone`, `change` and `zone`), and every key and string of
/// the examples.
pub fn focus_words(gap: &Gap, examples: &[Shown]) -> BTreeSet<String> {
    fn camel(word: &str, out: &mut BTreeSet<String>) {
        out.insert(word.to_lowercase());
        let mut part = String::new();
        for c in word.chars() {
            if c.is_uppercase() && !part.is_empty() {
                out.insert(std::mem::take(&mut part).to_lowercase());
            }
            part.push(c);
        }
        out.insert(part.to_lowercase());
    }
    fn walk(value: &Value, out: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    out.insert(key.to_lowercase());
                    walk(inner, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            Value::String(text) => {
                out.insert(text.to_lowercase());
            }
            _ => {}
        }
    }
    let mut words = BTreeSet::new();
    for word in gap.oracle.split(|c: char| !c.is_alphabetic()) {
        if word.len() >= 3 {
            words.insert(word.to_lowercase());
        }
    }
    for word in gap.forge.split(|c: char| !c.is_alphanumeric()) {
        if !word.is_empty() {
            camel(word, &mut words);
        }
    }
    for shown in examples {
        if let Ok(value) = serde_json::from_str::<Value>(&shown.ability) {
            walk(&value, &mut words);
        }
    }
    words
}

/// The longest text of the card's other abilities a prompt holds, in bytes.
const MAX_PROVEN: usize = 6_000;

/// What a line request holds besides the gap: the card's printed text and the abilities the translator proved.
#[derive(Debug, Clone, Copy)]
pub struct Card<'a> {
    /// The card's name.
    pub name: &'a str,
    /// The printed text of every face.
    pub oracle: &'a str,
    /// The translation as it is: `{"faces":[{"abilities":[...]}]}`.
    pub proven: &'a Value,
}

/// The proven abilities as compact JSON, each face on its line, cut at [`MAX_PROVEN`] bytes.
fn proven_text(proven: &Value) -> String {
    let faces = proven
        .get("faces")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut text = String::new();
    for (at, face) in faces.iter().enumerate() {
        let abilities = face.get("abilities").cloned().unwrap_or(Value::Null);
        let _ = writeln!(text, "face {at}: {abilities}");
    }
    if text.len() > MAX_PROVEN {
        let mut end = MAX_PROVEN;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str(" [cut]\n");
    }
    text
}

/// The request for one gapped line: the card's text, the line, its Forge lines, the gap, the proven abilities and the
/// examples; the system prompt is that of the kind.
pub fn request(model: &str, card: Card<'_>, gap: &Gap, examples: &[Shown]) -> Request {
    let reference = mtg_cards::reference::dsl_reference_focused(
        gap.kind.variants(),
        &focus_words(gap, examples),
        REFERENCE_BUDGET,
    );
    let mut text = format!("{CARD_MARKER}{}\n{reference}\n\n", card.name);
    let _ = writeln!(text, "Oracle text of the card:\n{}", card.oracle.trim_end());
    let _ = writeln!(text, "Oracle line to write: {}", gap.oracle);
    let _ = writeln!(text, "Forge script lines of it:\n{}", gap.forge);
    let _ = writeln!(text, "Why the translator stopped: {}", gap.key);
    let _ = write!(
        text,
        "The abilities already written for the card (proven; do not repeat them):\n{}",
        proven_text(card.proven)
    );
    if !examples.is_empty() {
        text.push_str("Examples of similar lines, nearest first, as the rules write them:\n");
        for shown in examples {
            let _ = writeln!(text, "- {} => {}", shown.text, shown.ability);
        }
    }
    let _ = write!(
        text,
        "Reply with the JSON of the one {} only, or {{\"missing\": \"...\"}}.",
        gap.kind.word()
    );
    Request {
        model: model.to_owned(),
        system: system(gap.kind),
        messages: vec![Message::user(text)],
        max_tokens: LINE_ANSWER_TOKENS,
    }
}

/// The message of a repair round: what is wrong with the answer, in the checker's words.
pub fn repair_message(problem: &str) -> String {
    format!(
        "That does not work: {problem}\nReply with the corrected JSON of the one ability only, or {{\"missing\": \"...\"}} \
         when the rules cannot say the line exactly."
    )
}

/// What an answer says: an ability, the engine piece the line needs, or why it is neither.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    /// One ability as JSON, checked against the DSL types.
    Ability(Value),
    /// The model says the DSL cannot express the line; the note names the missing engine piece.
    Missing(String),
    /// Not JSON, or not an ability; the words are for the model.
    Bad(String),
}

/// Reads an answer as one ability of the DSL or a `missing` note.
pub fn read_reply(answer: &str) -> Reply {
    let value: Value = match serde_json::from_str(extract_json(answer)) {
        Ok(value) => value,
        Err(error) => return Reply::Bad(format!("the answer is not JSON: {error}")),
    };
    if let Some(note) = value
        .as_object()
        .filter(|map| map.len() == 1)
        .and_then(|map| map.get("missing"))
        .and_then(Value::as_str)
    {
        let note = note.split_whitespace().collect::<Vec<_>>().join(" ");
        return Reply::Missing(note.chars().take(300).collect());
    }
    match serde_json::from_value::<Ability>(value.clone()) {
        Ok(_) => Reply::Ability(value),
        Err(error) => Reply::Bad(format!(
            "the answer is not an ability of the rules: {error}"
        )),
    }
}

/// Keywords structured outputs do not take; they are left out of a schema sent there.
const UNSUPPORTED: &[&str] = &[
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "pattern",
    "format",
];

fn strip(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in UNSUPPORTED {
                map.remove(*key);
            }
            if map.contains_key("properties") {
                map.insert("additionalProperties".into(), Value::Bool(false));
            }
            map.values_mut().for_each(strip);
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

/// Whether the definitions reachable from `root` refer back to one another.
fn recursive(root: &Value, defs: &Map<String, Value>) -> bool {
    fn visit(
        name: &str,
        defs: &Map<String, Value>,
        path: &mut Vec<String>,
        done: &mut BTreeSet<String>,
    ) -> bool {
        if path.iter().any(|seen| seen == name) {
            return true;
        }
        if done.contains(name) {
            return false;
        }
        let Some(def) = defs.get(name) else {
            return false;
        };
        path.push(name.to_owned());
        let mut inner = BTreeSet::new();
        refs(def, &mut inner);
        let cycle = inner.iter().any(|next| visit(next, defs, path, done));
        path.pop();
        done.insert(name.to_owned());
        cycle
    }
    let mut start = BTreeSet::new();
    refs(root, &mut start);
    let mut done = BTreeSet::new();
    start
        .iter()
        .any(|name| visit(name, defs, &mut Vec::new(), &mut done))
}

/// The schema of an answer about an ability of `kind` for structured outputs (`output_config.format`): the ability or a
/// `missing` note, without the keywords the API refuses. `None` when the types of the kind are recursive, which the API
/// does not take; the checker then reads the answer alone.
pub fn structured(kind: Kind) -> Option<Value> {
    structured_from(schema(kind))
}

/// [`structured`] of a schema with its definitions under `$defs`.
pub fn structured_from(mut root: Value) -> Option<Value> {
    let defs = match root.as_object_mut().and_then(|map| map.remove("$defs")) {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    if recursive(&root, &defs) {
        return None;
    }
    let missing = json!({
        "type": "object",
        "properties": {"missing": {"type": "string"}},
        "required": ["missing"],
        "additionalProperties": false,
    });
    let mut wrapped = json!({"anyOf": [root, missing], "$defs": defs});
    strip(&mut wrapped);
    Some(wrapped)
}
