//! The card rules as a JSON schema, built from the same parsed type sources as the reference, for servers that limit a
//! model's output to a schema (`docs/cards-dsl.md`, Proposer).
//!
//! Only the keywords the grammar converters of llama.cpp and Ollama read are used: `type`, `properties`, `required`,
//! `additionalProperties: false`, `enum`, `items`, `anyOf` and `$ref` to `$defs`. A schema checks shapes only: a value of
//! the right type in the wrong place (a player where an object is meant) still passes.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::parse::{Body, Field, Shape, TypeDef};
use super::{all_defs, reachable};

/// The type a rules file is.
const W3A_ROOT: &str = "Card";

/// Fields the checker fills in from the imported card, so a proposal may leave them out.
const W3A_FILLED: &[(&str, &str)] = &[
    ("Card", "oracle_id"),
    ("Card", "name"),
    ("Card", "impl_status"),
    ("Face", "name"),
    ("Face", "oracle_fingerprint"),
];

/// The schema of a card rules file: the root object with every type it reaches under `$defs`.
pub fn w3a_card_schema() -> Value {
    w3a_build().0
}

/// The schema and the type names no definition or primitive stands for.
fn w3a_build() -> (Value, Vec<String>) {
    let defs = all_defs();
    let by_name: BTreeMap<&str, &TypeDef> =
        defs.iter().map(|def| (def.name.as_str(), def)).collect();
    let mut builder = Builder {
        by_name: &by_name,
        unknown: Vec::new(),
    };
    let mut table = Map::new();
    let mut root = Value::Null;
    for name in reachable(&by_name) {
        let Some(def) = by_name.get(name.as_str()) else {
            continue;
        };
        let schema = builder.definition(def);
        if name == W3A_ROOT {
            root = schema;
        } else if name != "TokenFile" {
            table.insert(name, schema);
        }
    }
    if let Value::Object(object) = &mut root {
        object.insert("$defs".to_owned(), Value::Object(table));
    }
    (root, builder.unknown)
}

struct Builder<'a> {
    by_name: &'a BTreeMap<&'a str, &'a TypeDef>,
    unknown: Vec<String>,
}

impl Builder<'_> {
    /// The schema of one named type.
    fn definition(&mut self, def: &TypeDef) -> Value {
        match (def.name.as_str(), &def.body) {
            // A cost of mana alone is its text (`cost.rs`, `impl Deserialize for Cost`).
            ("Cost", Body::Tuple(items)) => {
                let list = items.first().map_or(Value::Null, |ty| self.ty(ty));
                json!({"anyOf": [{"type": "string"}, list]})
            }
            (_, Body::Enum(variants)) => self.enumeration(&def.name, variants),
            (_, Body::Struct(fields)) => self.object(&def.name, fields),
            (_, Body::Tuple(items)) => self.tuple(items),
            (_, Body::Unit) => json!({"type": "null"}),
            (_, Body::Text) => json!({"type": "string"}),
        }
    }

    /// An externally tagged enum: the unit variants as strings, every other variant as a one-key object.
    fn enumeration(&mut self, name: &str, variants: &[super::parse::Variant]) -> Value {
        let mut units = Vec::new();
        let mut options = Vec::new();
        for variant in variants {
            let payload = match &variant.shape {
                // A fixed amount is the bare number (`value.rs`, `impl Deserialize for Amount`).
                Shape::Tuple(_) if name == "Amount" && variant.name == "Fixed" => {
                    options.push(json!({"type": "integer"}));
                    continue;
                }
                Shape::Unit => {
                    units.push(Value::String(variant.name.clone()));
                    continue;
                }
                Shape::Tuple(items) => self.tuple(items),
                Shape::Fields(fields) => self.object("", fields),
            };
            options.push(json!({
                "type": "object",
                "properties": {variant.name.clone(): payload},
                "required": [variant.name],
                "additionalProperties": false,
            }));
        }
        if !units.is_empty() {
            options.insert(0, json!({"type": "string", "enum": units}));
        }
        match options.len() {
            1 => options.pop().unwrap_or(Value::Null),
            _ => json!({"anyOf": options}),
        }
    }

    /// An object with named fields; a field is required unless it may be left out.
    fn object(&mut self, owner: &str, fields: &[Field]) -> Value {
        let mut properties = Map::new();
        let mut required = Vec::new();
        for field in fields {
            let mut schema = self.ty(&field.ty);
            if field.nullable {
                schema = json!({"anyOf": [schema, {"type": "null"}]});
            }
            properties.insert(field.name.clone(), schema);
            let filled = W3A_FILLED.contains(&(owner, field.name.as_str()));
            if !field.optional && !filled {
                required.push(Value::String(field.name.clone()));
            }
        }
        let mut schema = json!({"type": "object", "properties": properties});
        if !required.is_empty() {
            schema["required"] = Value::Array(required);
        }
        schema["additionalProperties"] = Value::Bool(false);
        schema
    }

    /// A newtype is its inner type; a tuple of several is an array of them.
    fn tuple(&mut self, items: &[String]) -> Value {
        match items {
            [one] => self.ty(one),
            many => self.list_of(many),
        }
    }

    /// An array whose items are any of the types.
    fn list_of(&mut self, items: &[String]) -> Value {
        let mut options: Vec<Value> = Vec::new();
        for item in items {
            let schema = self.ty(item);
            if !options.contains(&schema) {
                options.push(schema);
            }
        }
        let item = match options.len() {
            1 => options.pop().unwrap_or(Value::Null),
            _ => json!({"anyOf": options}),
        };
        json!({"type": "array", "items": item})
    }

    /// The schema of a type as the parser renders it: `[T]`, `[T; 3]`, `(A, B)`, `T?`, a primitive or a name.
    fn ty(&mut self, ty: &str) -> Value {
        if let Some(inner) = ty.strip_suffix('?') {
            return self.ty(inner);
        }
        if let Some(inner) = ty.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            let item = inner.split_once(';').map_or(inner, |(item, _)| item).trim();
            let item = self.ty(item);
            return json!({"type": "array", "items": item});
        }
        if let Some(inner) = ty.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
            let items: Vec<String> = inner.split(", ").map(str::to_owned).collect();
            return self.list_of(&items);
        }
        match ty {
            "String" | "str" => json!({"type": "string"}),
            "bool" => json!({"type": "boolean"}),
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "ObjectId" => {
                json!({"type": "integer"})
            }
            name if self.by_name.contains_key(name) => json!({"$ref": format!("#/$defs/{name}")}),
            other => {
                self.unknown.push(other.to_owned());
                json!({})
            }
        }
    }
}

#[cfg(test)]
mod tests;
