//! One card: every line of its faces translated or recorded as a gap, each ability checked to parse as the DSL's `Ability`.

use mtg_cards::ability::Ability;
use serde_json::{Value, json};

use super::ability::ability_line;
use super::ctx::{Ctx, Tr, Unmapped, World, unmapped};
use super::keyword::keyword_abilities;
use super::replacements::replacement_line;
use super::statics::static_line;
use super::triggers::triggered;
use crate::forge::script::{Face, Params, Script};

/// The translation of one face.
#[derive(Debug, Clone, Default)]
pub struct FaceOut {
    /// The abilities translated, in the DSL's canonical JSON.
    pub abilities: Vec<Value>,
    /// The lines left out, each with the first construct that stopped it.
    pub gaps: Vec<Unmapped>,
    /// The lines of the face that state a rule (keywords, abilities, triggers, statics, replacements).
    pub lines: usize,
}

/// The translation of a script.
#[derive(Debug, Clone, Default)]
pub struct CardOut {
    /// The card's name as Scryfall writes it: the faces joined by ` // `.
    pub name: String,
    /// One entry for each face.
    pub faces: Vec<FaceOut>,
}

impl CardOut {
    /// The lines that state a rule.
    pub fn lines(&self) -> usize {
        self.faces.iter().map(|face| face.lines).sum()
    }

    /// The abilities translated.
    pub fn translated(&self) -> usize {
        self.faces.iter().map(|face| face.abilities.len()).sum()
    }

    /// Every gap.
    pub fn gaps(&self) -> impl Iterator<Item = &Unmapped> {
        self.faces.iter().flat_map(|face| &face.gaps)
    }

    /// The proposal in the shape a model answers with: `{"faces":[{"abilities":[...]}]}`.
    pub fn proposal(&self) -> Value {
        let faces: Vec<Value> = self
            .faces
            .iter()
            .map(|face| json!({ "abilities": face.abilities }))
            .collect();
        json!({ "faces": faces })
    }
}

/// Translates a script.
pub fn translate_script(world: &World, script: &Script) -> CardOut {
    let names: Vec<&str> = script.names().collect();
    CardOut {
        name: names.join(" // "),
        faces: script
            .faces
            .iter()
            .map(|face| translate_face(world, face))
            .collect(),
    }
}

/// Translates a face.
pub fn translate_face(world: &World, face: &Face) -> FaceOut {
    let mut ctx = Ctx::new(face, world);
    let mut out = FaceOut::default();
    for body in &face.keywords {
        for result in keyword_abilities(&mut ctx, body) {
            take(&mut out, result.map(Some));
        }
    }
    for body in &face.abilities {
        take(&mut out, ability_line(&mut ctx, body));
    }
    for group in trigger_groups(&face.triggers) {
        take(&mut out, merged_trigger(&mut ctx, &group).map(Some));
    }
    for body in &face.statics {
        take(&mut out, static_line(&mut ctx, body).map(Some));
    }
    for body in &face.replacements {
        take(&mut out, replacement_line(&mut ctx, body).map(Some));
    }
    out
}

/// The trigger lines grouped as the card prints them: lines that run the same part, one of them marked `Secondary$`,
/// are one printed trigger ("when this enters or attacks"); every other line stands alone.
fn trigger_groups(bodies: &[String]) -> Vec<Vec<&String>> {
    let mut groups: Vec<Vec<&String>> = Vec::new();
    for body in bodies {
        let params = Params::parse(body);
        let execute = params.get("Execute").map(str::to_owned);
        let secondary = params.get("Secondary") == Some("True");
        let same = |group: &Vec<&String>| {
            let first = Params::parse(group[0]);
            execute.is_some()
                && first.get("Execute") == execute.as_deref()
                && (secondary || group.iter().any(|b| Params::parse(b).get("Secondary") == Some("True")))
        };
        match groups.iter_mut().find(|group| same(group)) {
            Some(group) => group.push(body),
            None => groups.push(vec![body]),
        }
    }
    groups
}

/// One trigger from a group of lines: the event patterns of the lines joined as alternatives when the rest of the
/// abilities is the same.
fn merged_trigger(ctx: &mut Ctx, group: &[&String]) -> Tr<Value> {
    let mut abilities = Vec::new();
    for body in group {
        abilities.push(triggered(ctx, body)?);
    }
    let mut merged = abilities.remove(0);
    for other in abilities {
        let (Some(mine), Some(theirs)) = (
            merged.get_mut("Triggered").and_then(Value::as_object_mut),
            other.get("Triggered").and_then(Value::as_object),
        ) else {
            return unmapped("trigger:merge-shape");
        };
        let rest = |map: &serde_json::Map<String, Value>| {
            map.iter()
                .filter(|(key, _)| key.as_str() != "when")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>()
        };
        if rest(mine) != rest(theirs) {
            return unmapped("trigger:Secondary-with-different-effects");
        }
        let (Some(first), Some(second)) = (mine.get("when").cloned(), theirs.get("when").cloned()) else {
            return unmapped("trigger:merge-shape");
        };
        let mut alternatives = match first.get("AnyOf") {
            Some(Value::Array(list)) => list.clone(),
            _ => vec![first],
        };
        alternatives.push(second);
        mine.insert("when".to_owned(), json!({ "AnyOf": alternatives }));
    }
    Ok(merged)
}

/// Records the result of one line: the ability when it parses, else the gap.
fn take(out: &mut FaceOut, result: Tr<Option<Value>>) {
    out.lines += 1;
    match result.and_then(|value| value.map(checked).transpose()) {
        Ok(Some(ability)) => out.abilities.push(ability),
        Ok(None) => out.lines -= 1,
        Err(gap) => out.gaps.push(gap),
    }
}

/// The ability in canonical form, or the gap that names why the DSL does not read it.
fn checked(value: Value) -> Tr<Value> {
    match serde_json::from_value::<Ability>(value) {
        Ok(ability) => serde_json::to_value(&ability)
            .or_else(|_| unmapped("invalid:serialize")),
        Err(error) => unmapped(format!("invalid:{}", reason(&error.to_string()))),
    }
}

/// The part of a serde error that names the construct: the unknown variant or field, or the first words.
fn reason(message: &str) -> String {
    message
        .split(", expected")
        .next()
        .unwrap_or(message)
        .chars()
        .take(60)
        .collect()
}

/// The event or mode a line of a face names, for a gap's name.
pub fn line_name(body: &str) -> String {
    let params = Params::parse(body);
    ["Mode", "Event", "SP", "AB", "DB"]
        .iter()
        .find_map(|key| params.get(key))
        .unwrap_or("?")
        .to_owned()
}
