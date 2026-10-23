//! The comparison of one card: the verbs Forge names for it against the verbs of our rules.

use std::collections::BTreeSet;

use mtg_cards::assemble::PLAIN_MANA_ABILITY;
use mtg_cards::card::Card;
use mtg_cards::validate::used_primitives;

use super::map::{Lookup, Map, norm};
use super::script::{Item, Kind};

/// The verbs of a card's rules as the comparison names them: every primitive of every modelled ability, normalised
/// (`Effect.Move`, `Keyword.Flying`, `Trigger.ZoneChange`). A plain mana ability counts as a mana effect with a tap cost.
pub fn verbs_of(card: &Card) -> BTreeSet<String> {
    let mut verbs = BTreeSet::new();
    for (_, ability) in card.abilities().filter(|(_, ability)| !ability.is_manual()) {
        for name in used_primitives(ability) {
            if name == PLAIN_MANA_ABILITY {
                verbs.extend(["Ability.Activated", "Effect.AddMana", "Cost.Tap"].map(norm));
            }
            verbs.insert(norm(&name));
        }
    }
    verbs
}

/// How an item of a script stands against our rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// One of the verbs the map names for it is in our rules.
    Read,
    /// The map names verbs for it and our rules have none of them.
    Missing,
    /// The DSL has no verb for it.
    NoVerb,
    /// The map does not name it.
    Unmapped,
}

/// One item of a script, or one key of an item, set against our rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The family.
    pub kind: Kind,
    /// The API, mode, event, keyword or cost part.
    pub name: String,
    /// For a finding about an exception: the key that asks for a verb of its own.
    pub key: Option<String>,
    /// How it stands.
    pub status: Status,
    /// The verbs of ours that stand for it (none for `NoVerb` and `Unmapped`).
    pub expect: Vec<String>,
    /// The keys of its line that vary the verb; the keys the map tells uses apart by carry their value (`Destination=Exile`).
    pub keys: Vec<String>,
}

#[cfg(test)]
mod standard;
#[cfg(test)]
mod tests;

/// Sets every item against the verbs of our rules; bookkeeping the map skips gives no finding. A mapped item that our
/// rules read also gives a finding for each of its keys that asks for a verb of its own and finds none.
pub fn compare(items: &[Item], ours: &BTreeSet<String>, map: &Map) -> Vec<Finding> {
    let has = |verbs: &[String]| verbs.iter().any(|verb| ours.contains(&norm(verb)));
    let mut findings = Vec::new();
    for item in items {
        let (status, expect) = match map.lookup(item) {
            Lookup::Skip => continue,
            Lookup::Verbs(verbs) if has(&verbs) => (Status::Read, verbs),
            Lookup::Verbs(verbs) => (Status::Missing, verbs),
            Lookup::NoVerb => (Status::NoVerb, Vec::new()),
            Lookup::Unmapped => (Status::Unmapped, Vec::new()),
        };
        let mut keys = item.params.meaningful_keys();
        keys.extend(map.variants(item));
        if matches!(status, Status::Read | Status::Missing) {
            for wanted in map.wanted(item) {
                if !has(&wanted.verbs) {
                    findings.push(Finding {
                        kind: item.kind,
                        name: item.name.clone(),
                        key: Some(wanted.key),
                        status: Status::Missing,
                        expect: wanted.verbs,
                        keys: Vec::new(),
                    });
                }
            }
        }
        findings.push(Finding {
            kind: item.kind,
            name: item.name.clone(),
            key: None,
            status,
            expect,
            keys,
        });
    }
    findings
}
