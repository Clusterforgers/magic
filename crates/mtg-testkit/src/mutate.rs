//! Single-fault mutants of a card's rules, for measuring how many faults a check catches (mutation analysis).
//!
//! Each kind injects one small fault of the sort a translator or writer makes: a changed number, a dropped condition,
//! "may" or "another", a dropped filter, "you" swapped with "opponents", a dropped or changed duration, a dropped
//! ability. A mutant is kept only when it still loads as rules and differs from the original.

use mtg_cards::card::Card;
use serde_json::Value;

/// The kind of fault a mutant carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MutationKind {
    /// A number of the rules is one higher (0 becomes 1).
    Number,
    /// A condition (an intervening "if", a `condition`, an `If` step) is dropped.
    Condition,
    /// A "may" is dropped: the steps are done without asking, or an `optional` flag is cleared.
    May,
    /// "Another" (`Other`) is dropped from a selector.
    Another,
    /// One filter (`Not`, a type, subtype, colour or `Token`) is dropped from a selector of two or more.
    Filter,
    /// "You" and "opponents" are swapped.
    Player,
    /// A duration is dropped or changed.
    Duration,
    /// One ability of a face is dropped.
    Clause,
}

impl MutationKind {
    /// Every kind, in a fixed order.
    pub const ALL: [MutationKind; 8] = [
        MutationKind::Number,
        MutationKind::Condition,
        MutationKind::May,
        MutationKind::Another,
        MutationKind::Filter,
        MutationKind::Player,
        MutationKind::Duration,
        MutationKind::Clause,
    ];

    /// The kind's name in reports.
    pub fn name(self) -> &'static str {
        match self {
            MutationKind::Number => "number",
            MutationKind::Condition => "condition",
            MutationKind::May => "may",
            MutationKind::Another => "another",
            MutationKind::Filter => "filter",
            MutationKind::Player => "player",
            MutationKind::Duration => "duration",
            MutationKind::Clause => "clause",
        }
    }
}

/// One mutant: its kind, where the fault sits (a JSON pointer into the card) and the faulty rules.
#[derive(Debug, Clone, PartialEq)]
pub struct Mutant {
    /// The fault.
    pub kind: MutationKind,
    /// The JSON pointer of the changed node in the original card.
    pub at: String,
    /// The mutated rules.
    pub card: Card,
}

/// Keys whose values are bookkeeping, not rules: a fault there changes nothing a player sees.
const SKIP: [&str; 4] = ["span", "oracle_fingerprint", "oracle_id", "name"];
/// Selector atoms a `Filter` mutant may drop.
const FILTERS: [&str; 5] = ["Not", "Type", "Subtype", "Color", "Supertype"];
/// Effects that ask the player first.
const MAY: &str = "May";

/// A candidate edit: where it applies and how.
#[derive(Debug, Clone)]
enum Edit {
    /// Replace the node with this value.
    Set(String, Value),
    /// Remove the key from its object.
    RemoveKey(String),
    /// Remove the array element (pointer to the element).
    RemoveElement(String),
    /// Replace the array element by these elements.
    Splice(String, Vec<Value>),
}

impl Edit {
    fn at(&self) -> &str {
        match self {
            Edit::Set(at, _)
            | Edit::RemoveKey(at)
            | Edit::RemoveElement(at)
            | Edit::Splice(at, _) => at,
        }
    }
}

/// Escapes a key for a JSON pointer.
fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Every node of the value with its pointer, its parent key (for an object member) and whether it is an array element
/// of an array of at least `n` elements.
fn walk<'a>(
    value: &'a Value,
    at: &str,
    key: Option<&str>,
    out: &mut Vec<Node<'a>>,
    siblings: usize,
) {
    out.push(Node {
        at: at.to_owned(),
        key: key.map(str::to_owned),
        value,
        siblings,
    });
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if SKIP.contains(&k.as_str()) {
                    continue;
                }
                walk(v, &format!("{at}/{}", escape(k)), Some(k), out, 0);
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                walk(v, &format!("{at}/{i}"), None, out, items.len());
            }
        }
        _ => {}
    }
}

/// A node of the walk.
struct Node<'a> {
    at: String,
    key: Option<String>,
    value: &'a Value,
    /// The length of the array the node is an element of; 0 for an object member or the root.
    siblings: usize,
}

/// The single key of a one-member object.
fn tag(value: &Value) -> Option<(&str, &Value)> {
    match value {
        Value::Object(map) if map.len() == 1 => map.iter().next().map(|(k, v)| (k.as_str(), v)),
        _ => None,
    }
}

/// The candidate edits of one kind, in document order.
fn candidates(root: &Value, kind: MutationKind) -> Vec<Edit> {
    let mut nodes = Vec::new();
    walk(root, "", None, &mut nodes, 0);
    let mut edits = Vec::new();
    for node in &nodes {
        let element = node.siblings > 0;
        match kind {
            MutationKind::Number => {
                if let Some(n) = node.value.as_i64() {
                    edits.push(Edit::Set(
                        node.at.clone(),
                        Value::from(if n == 0 { 1 } else { n + 1 }),
                    ));
                }
            }
            MutationKind::Condition => {
                if matches!(node.key.as_deref(), Some("if" | "condition")) {
                    edits.push(Edit::RemoveKey(node.at.clone()));
                }
                if element && let Some(("If", body)) = tag(node.value) {
                    let then = body
                        .get("then")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    edits.push(Edit::Splice(node.at.clone(), then));
                }
            }
            MutationKind::May => {
                if element && let Some((MAY, body)) = tag(node.value) {
                    let effects = body
                        .get("effects")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    edits.push(Edit::Splice(node.at.clone(), effects));
                }
                if node.key.as_deref() == Some("optional") && node.value == &Value::Bool(true) {
                    edits.push(Edit::RemoveKey(node.at.clone()));
                }
            }
            MutationKind::Another => {
                if element && node.value.as_str() == Some("Other") {
                    edits.push(Edit::RemoveElement(node.at.clone()));
                }
            }
            MutationKind::Filter => {
                let atom = node.value.as_str() == Some("Token")
                    || tag(node.value).is_some_and(|(k, _)| FILTERS.contains(&k));
                if node.siblings >= 2 && atom {
                    edits.push(Edit::RemoveElement(node.at.clone()));
                }
            }
            MutationKind::Player => match node.value.as_str() {
                Some("You") => edits.push(Edit::Set(node.at.clone(), Value::from("Opponents"))),
                Some("Opponents") => edits.push(Edit::Set(node.at.clone(), Value::from("You"))),
                _ => {}
            },
            MutationKind::Duration => {
                if node.key.as_deref() == Some("duration") {
                    edits.push(Edit::RemoveKey(node.at.clone()));
                    let other = if node.value.as_str() == Some("UntilEndOfTurn") {
                        "WhileSourceOnBattlefield"
                    } else {
                        "UntilEndOfTurn"
                    };
                    edits.push(Edit::Set(node.at.clone(), Value::from(other)));
                }
            }
            MutationKind::Clause => {
                let ability = node.at.starts_with("/faces/")
                    && node.at.split('/').count() == 5
                    && node.at.split('/').nth(3) == Some("abilities");
                if ability && node.siblings >= 2 {
                    edits.push(Edit::RemoveElement(node.at.clone()));
                }
            }
        }
    }
    edits
}

/// The parent pointer and last token of a pointer.
fn split(at: &str) -> Option<(&str, String)> {
    let (parent, last) = at.rsplit_once('/')?;
    Some((parent, last.replace("~1", "/").replace("~0", "~")))
}

/// The value with the edit applied, or `None` when the pointer does not resolve.
fn apply(root: &Value, edit: &Edit) -> Option<Value> {
    let mut value = root.clone();
    match edit {
        Edit::Set(at, new) => *value.pointer_mut(at)? = new.clone(),
        Edit::RemoveKey(at) => {
            let (parent, key) = split(at)?;
            value.pointer_mut(parent)?.as_object_mut()?.remove(&key)?;
        }
        Edit::RemoveElement(at) => {
            let (parent, index) = split(at)?;
            let index: usize = index.parse().ok()?;
            let items = value.pointer_mut(parent)?.as_array_mut()?;
            (index < items.len()).then(|| items.remove(index))?;
        }
        Edit::Splice(at, with) => {
            let (parent, index) = split(at)?;
            let index: usize = index.parse().ok()?;
            let items = value.pointer_mut(parent)?.as_array_mut()?;
            if index >= items.len() {
                return None;
            }
            items.splice(index..=index, with.iter().cloned());
        }
    }
    Some(value)
}

/// One mutant of a kind: the first candidate, counted from `seed` round the candidates, that still loads as rules and
/// differs from the original; `None` when the rules hold no such node.
pub fn mutant(card: &Card, kind: MutationKind, seed: u64) -> Option<Mutant> {
    let root = serde_json::to_value(card).ok()?;
    let edits = candidates(&root, kind);
    let n = edits.len();
    let start = usize::try_from(seed).unwrap_or(0) % n.max(1);
    (0..n).map(|i| &edits[(start + i) % n]).find_map(|edit| {
        let changed = apply(&root, edit)?;
        let mutated: Card = serde_json::from_value(changed).ok()?;
        (serde_json::to_value(&mutated).ok()? != root).then(|| Mutant {
            kind,
            at: edit.at().to_owned(),
            card: mutated,
        })
    })
}

/// One mutant of every kind the rules allow.
pub fn mutants(card: &Card, seed: u64) -> Vec<Mutant> {
    MutationKind::ALL
        .iter()
        .filter_map(|&kind| mutant(card, kind, seed))
        .collect()
}
