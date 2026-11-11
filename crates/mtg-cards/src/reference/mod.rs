//! The DSL reference: the type tree of a rules file and its vocabulary as compact text, generated from the source of the
//! types so it cannot drift from them (`docs/cards-dsl.md`, Proposals and drafts).

mod lex;
mod parse;
mod w3a_schema;

pub use w3a_schema::w3a_card_schema;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;

use parse::{Body, Field, Shape, TypeDef, Variant, type_defs};

/// The sources of the types a rules file is made of.
const SOURCES: &[&str] = &[
    include_str!("../card.rs"),
    include_str!("../ability.rs"),
    include_str!("../effect.rs"),
    include_str!("../did.rs"),
    include_str!("../trigger.rs"),
    include_str!("../cost.rs"),
    include_str!("../refs.rs"),
    include_str!("../value.rs"),
    include_str!("../modify.rs"),
    include_str!("../mana.rs"),
    include_str!("../mana_trigger.rs"),
    include_str!("../spend.rs"),
    include_str!("../keyword.rs"),
    include_str!("../companion.rs"),
    include_str!("../../../mtg-core/src/card.rs"),
    include_str!("../../../mtg-core/src/color.rs"),
    include_str!("../../../mtg-core/src/mana.rs"),
];

/// The types a rules file starts from: a card, and a predefined token.
const ROOTS: &[&str] = &["Card", "TokenFile"];

/// The longest doc line kept after a name.
const DOC_LIMIT: usize = 80;

/// The width unit-only enums are wrapped at.
const WRAP: usize = 110;

const NOTATION: &str = "\
DSL reference: the JSON shape of a rules file, generated from the Rust types (docs/cards-dsl.md).
Notation: a `struct` is an object; an `enum` value is the variant's name as a string when it has no payload (\"Tap\"),
{\"Name\": value} with one value, {\"Name\": [a, b]} with several, {\"Name\": {\"field\": value}} with named fields.
`[T]` is an array, `T?` and `field?:` may be left out (left out, a field is `= v` when it shows one, else empty, false or 0),
so {\"Draw\": {}} is a draw of one card for you. `type A = B` is a name for B. `// text` is the variant's meaning.
Exceptions: a fixed Amount is the bare number (3); a Cost of mana alone is the string \"{1}{R}\", any other Cost is an array of CostItem
([\"Tap\", {\"PayLife\": 1}]); a Selector is an array of Pred that must all hold; a Subtype is its name (\"Elf\"); mana is written as printed (\"{2}{R}\").";

/// The reference: the notation, then every type reachable from a card, the card first.
pub fn dsl_reference() -> String {
    let defs = all_defs();
    let by_name: BTreeMap<&str, &TypeDef> =
        defs.iter().map(|def| (def.name.as_str(), def)).collect();
    let mut text = String::from(NOTATION);
    text.push('\n');
    for name in reachable(&by_name) {
        if let Some(def) = by_name.get(name.as_str()) {
            text.push('\n');
            text.push_str(&render(def));
        }
    }
    text
}

/// Every public type of the sources.
fn all_defs() -> Vec<TypeDef> {
    SOURCES
        .iter()
        .flat_map(|source| type_defs(source))
        .collect()
}

/// An enum with more variants than this shows only the variants a focused reference finds words for.
const FOCUS_ENUM: usize = 12;

/// A focused reference for one line: the notation, the asked variants of `Ability`, then the types they reach, breadth
/// first, while the text stays within `budget` bytes. A large enum shows only the variants whose name is in `words`
/// (lowercase, whole or as all of its camel-case words); the others are counted, not shown.
pub fn dsl_reference_focused(variants: &[&str], words: &BTreeSet<String>, budget: usize) -> String {
    static DEFS: std::sync::OnceLock<Vec<TypeDef>> = std::sync::OnceLock::new();
    let defs = DEFS.get_or_init(all_defs);
    let by_name: BTreeMap<&str, &TypeDef> =
        defs.iter().map(|def| (def.name.as_str(), def)).collect();
    let mut text = String::from(NOTATION);
    text.push_str("\nOnly the parts of the reference this line likely needs are shown.\n\nenum Ability (the variants asked for)\n");
    let mut queue: VecDeque<String> = VecDeque::new();
    if let Some(Body::Enum(all)) = by_name.get("Ability").map(|def| &def.body) {
        for variant in all.iter().filter(|v| variants.contains(&v.name.as_str())) {
            let _ = writeln!(text, "  {}", variant_line(variant));
            queue.extend(words_in(shape_types(&variant.shape)));
        }
    }
    let mut seen: BTreeSet<String> = queue.iter().cloned().collect();
    let named = |name: &str| {
        let lower = name.to_lowercase();
        words.contains(&lower) || words_of(name).split(' ').all(|word| words.contains(word))
    };
    while let Some(name) = queue.pop_front() {
        let Some(def) = by_name.get(name.as_str()) else {
            continue;
        };
        let (rendered, used) = match &def.body {
            Body::Enum(all) if all.len() > FOCUS_ENUM => {
                let kept: Vec<&Variant> = all.iter().filter(|v| named(&v.name)).collect();
                let mut part = format!(
                    "enum {}{} ({} of {} variants shown)\n",
                    def.name,
                    doc_suffix(&def.doc),
                    kept.len(),
                    all.len()
                );
                let mut used = Vec::new();
                for variant in kept {
                    let _ = writeln!(part, "  {}", variant_line(variant));
                    used.extend(words_in(shape_types(&variant.shape)));
                }
                (part, used)
            }
            _ => (render(def), type_names(def)),
        };
        if text.len() + rendered.len() + 1 > budget {
            continue;
        }
        text.push('\n');
        text.push_str(&rendered);
        for next in used {
            if by_name.contains_key(next.as_str()) && seen.insert(next.clone()) {
                queue.push_back(next);
            }
        }
    }
    text
}

/// The names of the types reachable from the roots, in breadth-first order.
fn reachable(by_name: &BTreeMap<&str, &TypeDef>) -> Vec<String> {
    let roots: Vec<String> = ROOTS.iter().map(|name| (*name).to_owned()).collect();
    reachable_from(by_name, &roots)
}

/// The names of the types reachable from `roots`, in breadth-first order.
fn reachable_from(by_name: &BTreeMap<&str, &TypeDef>, roots: &[String]) -> Vec<String> {
    let roots: Vec<String> = roots
        .iter()
        .filter(|name| by_name.contains_key(name.as_str()))
        .cloned()
        .collect();
    let mut seen: BTreeSet<String> = roots.iter().cloned().collect();
    let mut queue: VecDeque<String> = roots.into_iter().collect();
    let mut order = Vec::new();
    while let Some(name) = queue.pop_front() {
        let Some(def) = by_name.get(name.as_str()) else {
            continue;
        };
        for used in type_names(def) {
            if by_name.contains_key(used.as_str()) && seen.insert(used.clone()) {
                queue.push_back(used);
            }
        }
        order.push(name);
    }
    order
}

/// The capitalised words of the types a definition uses.
fn type_names(def: &TypeDef) -> Vec<String> {
    let mut types: Vec<&String> = Vec::new();
    match &def.body {
        Body::Enum(variants) => {
            for variant in variants {
                types.extend(shape_types(&variant.shape));
            }
        }
        Body::Struct(list) => types.extend(list.iter().map(|field| &field.ty)),
        Body::Tuple(items) => types.extend(items),
        Body::Unit | Body::Text => {}
    }
    words_in(types)
}

/// The type texts of a variant's payload.
fn shape_types(shape: &Shape) -> Vec<&String> {
    match shape {
        Shape::Unit => Vec::new(),
        Shape::Tuple(items) => items.iter().collect(),
        Shape::Fields(list) => list.iter().map(|field| &field.ty).collect(),
    }
}

/// The capitalised words of type texts (`Option<Vec<Effect>>` gives `Option`, `Vec`, `Effect`).
fn words_in(types: Vec<&String>) -> Vec<String> {
    types
        .into_iter()
        .flat_map(|ty| ty.split(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|word| word.starts_with(char::is_uppercase))
        .map(str::to_owned)
        .collect()
}

/// One type as lines of text.
fn render(def: &TypeDef) -> String {
    let doc = doc_suffix(&def.doc);
    match &def.body {
        Body::Enum(variants) => {
            let mut text = format!("enum {}{doc}\n", def.name);
            let mut plain: Vec<&str> = Vec::new();
            for variant in variants {
                if is_plain(variant) {
                    plain.push(&variant.name);
                    continue;
                }
                flush(&mut plain, &mut text);
                let _ = writeln!(text, "  {}", variant_line(variant));
            }
            flush(&mut plain, &mut text);
            text
        }
        Body::Struct(fields) => format!("struct {} {}{doc}\n", def.name, field_list(fields)),
        Body::Tuple(items) => format!("type {} = {}{doc}\n", def.name, tuple_text(items)),
        Body::Unit => format!("struct {}{doc}\n", def.name),
        Body::Text => format!("type {} = string{doc}\n", def.name),
    }
}

/// Whether a variant is its name alone: no payload, and no doc line beyond the name in words.
fn is_plain(variant: &Variant) -> bool {
    matches!(variant.shape, Shape::Unit) && {
        let words = words_of(&variant.name);
        variant.doc.is_empty() || variant.doc.to_lowercase() == words
    }
}

/// `FirstStrike` as `first strike`.
fn words_of(name: &str) -> String {
    let mut text = String::new();
    for (at, c) in name.chars().enumerate() {
        if c.is_uppercase() && at > 0 {
            text.push(' ');
        }
        text.extend(c.to_lowercase());
    }
    text
}

/// Writes the plain variants collected so far as wrapped lines and clears them.
fn flush(plain: &mut Vec<&str>, text: &mut String) {
    if !plain.is_empty() {
        text.push_str(&wrap(plain));
        text.push('\n');
        plain.clear();
    }
}

fn tuple_text(items: &[String]) -> String {
    match items {
        [one] => one.clone(),
        many => format!("({})", many.join(", ")),
    }
}

/// `  // doc` for a doc line, or nothing.
fn doc_suffix(doc: &str) -> String {
    if doc.is_empty() {
        return String::new();
    }
    let mut kept: String = doc.chars().take(DOC_LIMIT).collect();
    if kept.len() < doc.len() {
        kept.push_str("...");
    }
    format!("  // {kept}")
}

fn variant_line(variant: &Variant) -> String {
    let shape = match &variant.shape {
        Shape::Unit => variant.name.clone(),
        Shape::Tuple(items) => format!("{}({})", variant.name, items.join(", ")),
        Shape::Fields(fields) => format!("{} {}", variant.name, field_list(fields)),
    };
    format!("{shape}{}", doc_suffix(&variant.doc))
}

/// `{ a: T, b?: U = 1 }`.
fn field_list(fields: &[Field]) -> String {
    if fields.is_empty() {
        return "{}".to_owned();
    }
    let parts: Vec<String> = fields
        .iter()
        .map(|field| {
            let mark = if field.optional { "?" } else { "" };
            let default = field
                .default
                .as_ref()
                .map_or_else(String::new, |value| format!(" = {value}"));
            format!("{}{mark}: {}{default}", field.name, field.ty)
        })
        .collect();
    format!("{{ {} }}", parts.join(", "))
}

/// Names joined by ` | `, wrapped to [`WRAP`] columns with a two-space indent.
fn wrap(names: &[&str]) -> String {
    let mut lines = Vec::new();
    let mut line = String::from("  ");
    for (at, name) in names.iter().enumerate() {
        let piece = if at == 0 {
            (*name).to_owned()
        } else {
            format!(" | {name}")
        };
        if line.len() + piece.len() > WRAP && line.trim() != "" {
            lines.push(std::mem::replace(&mut line, String::from("  ")));
            line.push_str(name);
        } else {
            line.push_str(&piece);
        }
    }
    lines.push(line);
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    //! Goal: the reference lists every type a rules file is made of with its serde names, defaults and doc lines, stays
    //! under a size budget, and the vocabulary of every hand-written file is in it.
    use std::collections::BTreeSet;

    use serde_json::Value;

    use super::*;

    /// The reference in bytes may not grow past this without a decision (about 30 thousand tokens in a prompt). It only
    /// feeds the local model; the Forge path does not use it.
    const BUDGET: usize = 122_000;

    #[test]
    fn the_reference_stays_under_its_budget() {
        let text = dsl_reference();
        assert!(
            text.len() <= BUDGET,
            "the reference is {} bytes, the budget {BUDGET}",
            text.len()
        );
    }

    #[test]
    fn the_roots_and_their_parts_are_there() {
        let text = dsl_reference();
        for needle in [
            "struct Card {",
            "enum Ability",
            "enum Effect",
            "enum TriggerPattern",
            "enum CostItem",
            "enum Keyword",
            "enum CardType",
            "struct TokenFile",
        ] {
            assert!(text.contains(needle), "missing `{needle}`");
        }
    }

    #[test]
    fn defaults_renames_and_optional_fields_are_shown() {
        let text = dsl_reference();
        // `#[serde(default = "Amount::one")]`, `#[serde(rename = "as")]` and an `Option` field.
        assert!(text.contains("count?: Amount = 1"), "default amount");
        assert!(text.contains("as?: "), "renamed field");
        assert!(text.contains("tapped?: bool"), "default bool");
    }

    #[test]
    fn type_names_are_unique_across_the_sources() {
        let defs = all_defs();
        let mut seen = BTreeSet::new();
        for def in &defs {
            assert!(
                seen.insert(def.name.clone()),
                "{} is defined twice",
                def.name
            );
        }
    }

    /// Every object key a rules file uses: the names of fields and of variants with a payload.
    fn keys(value: &Value, found: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    found.insert(key.clone());
                    keys(inner, found);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| keys(item, found)),
            _ => {}
        }
    }

    #[test]
    fn every_key_of_every_hand_written_file_is_in_the_reference() {
        let text = dsl_reference();
        let words: BTreeSet<&str> = text
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .collect();
        let library = crate::builtin::library().expect("the embedded rules load");
        let mut used = BTreeSet::new();
        for card in library.cards() {
            let value = serde_json::to_value(card).expect("a card serializes");
            keys(&value, &mut used);
        }
        let missing: Vec<&String> = used
            .iter()
            .filter(|key| !words.contains(key.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "missing from the reference: {missing:?}"
        );
    }
}
