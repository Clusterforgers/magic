//! The report: the numbers of the comparison, the verbs Forge names that our rules lack, and the unread tail grouped by
//! Forge API and key.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use mtg_cards::coverage::{Bucket, RulesSource};
use mtg_cards::support::Support;

use super::compare::{Finding, Status};
use super::script::Kind;

/// One stored card that Forge has, set against its script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardCheck {
    /// The card's name.
    pub name: String,
    /// Where its rules came from.
    pub source: RulesSource,
    /// How well the game plays it.
    pub support: Support,
    /// Whether a clause is left to the table or the rules are unusable.
    pub unread: bool,
    /// The items of its script set against our rules.
    pub findings: Vec<Finding>,
    /// The verbs of our rules, normalised.
    pub ours: Vec<String>,
}

impl CardCheck {
    /// The findings that are not read, each with its place in the report.
    fn gaps(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|finding| finding.status != Status::Read)
    }
}

/// The comparison of all stored cards.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Survey {
    /// Distinct stored cards.
    pub stored: usize,
    /// Names of the stored cards that Forge has no script for.
    pub absent: Vec<String>,
    /// The cards Forge has, in name order.
    pub checks: Vec<CardCheck>,
}

/// How many items of the scripts stand in each way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Items {
    /// All items except bookkeeping.
    pub all: usize,
    /// Items with a verb in our rules.
    pub read: usize,
    /// Items whose mapped verbs our rules lack.
    pub missing: usize,
    /// Items the DSL has no verb for.
    pub no_verb: usize,
    /// Items the map does not name.
    pub unmapped: usize,
}

/// A verb that Forge names and some cards' rules lack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingVerb {
    /// The verbs that stand for it, then the Forge item (and key) in brackets.
    pub label: String,
    /// Cards whose script names it and whose rules lack it.
    pub cards: usize,
    /// Of them, cards whose clauses are all read.
    pub read_cards: usize,
    /// The first few cards that lack it.
    pub examples: Vec<String>,
    /// The first few of the cards whose clauses are all read.
    pub read_examples: Vec<String>,
}

/// A group of the unread tail: a Forge API (or mode, event, keyword, cost part) with the keys its lines carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The family.
    pub kind: Kind,
    /// The API, mode, event, keyword or cost part.
    pub name: String,
    /// How the map stands on it.
    pub status: Status,
    /// Unread cards whose script has it unread.
    pub cards: usize,
    /// Of them, cards that have no other group.
    pub alone: usize,
    /// The keys of the group with the cards that carry each, most first.
    pub keys: Vec<(String, usize)>,
}

impl Group {
    /// The group as the report writes it: an effect by its API, anything else after its family.
    pub fn label(&self) -> String {
        match self.kind {
            Kind::Effect => self.name.clone(),
            kind => format!("{} {}", kind.word(), self.name),
        }
    }
}

/// The unread tail.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tail {
    /// Cards with a clause left to the table or rules that are unusable.
    pub cards: usize,
    /// Of them, cards that have a group.
    pub grouped: usize,
    /// The groups, most cards first.
    pub groups: Vec<Group>,
    /// For a number of readers (the groups from the top): the grouped cards whose groups all fall inside.
    pub covered: Vec<(usize, usize)>,
}

/// The cards of the unread tail a ranking is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Every card with a clause left to the table.
    All,
    /// Those whose rules read at least one ability: what is left is the unread clauses.
    Partly,
    /// Those whose rules read nothing: every item of the script is open.
    Nothing,
}

impl Scope {
    /// The scopes in report order.
    pub const ALL: [Scope; 3] = [Scope::All, Scope::Partly, Scope::Nothing];

    fn holds(self, check: &CardCheck) -> bool {
        check.unread
            && match self {
                Scope::All => true,
                Scope::Partly => !check.ours.is_empty(),
                Scope::Nothing => check.ours.is_empty(),
            }
    }

    fn title(self) -> &'static str {
        match self {
            Scope::All => "all cards with a clause left to the table",
            Scope::Partly => "those whose rules read some abilities",
            Scope::Nothing => "those whose rules read nothing",
        }
    }
}

/// How many rows the report shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rows {
    /// Rows of each ranking.
    pub top: usize,
    /// Keys named in a row of the tail.
    pub keys: usize,
}

/// A group of fewer cards lists every key.
const DEFINING_MIN_CARDS: usize = 5;

/// The numbers of readers the report prices.
const READERS: [usize; 7] = [1, 3, 5, 10, 20, 50, 100];

impl Survey {
    /// Counts the items of the scripts of the cards Forge has.
    pub fn items(&self) -> Items {
        let mut items = Items::default();
        for finding in self.checks.iter().flat_map(|check| &check.findings) {
            if finding.key.is_some() {
                continue;
            }
            items.all += 1;
            match finding.status {
                Status::Read => items.read += 1,
                Status::Missing => items.missing += 1,
                Status::NoVerb => items.no_verb += 1,
                Status::Unmapped => items.unmapped += 1,
            }
        }
        items
    }

    /// Cards and the buckets of their state, in report order.
    pub fn states(&self) -> Vec<(Bucket, usize)> {
        // A card waits for a read only when its library holds drafts, which a Forge check never loads.
        Bucket::ALL
            .into_iter()
            .filter(|bucket| *bucket != Bucket::Waiting)
            .map(|bucket| {
                let cards = self
                    .checks
                    .iter()
                    .filter(|check| Bucket::of(check.support) == bucket)
                    .count();
                (bucket, cards)
            })
            .collect()
    }

    /// The verbs Forge names for a card that its rules lack (the mapped ones), by cards, most first.
    pub fn missing_verbs(&self) -> Vec<MissingVerb> {
        let mut found: BTreeMap<String, MissingVerb> = BTreeMap::new();
        for check in &self.checks {
            let labels: BTreeSet<String> = check
                .gaps()
                .filter(|finding| finding.status == Status::Missing)
                .map(label_of)
                .collect();
            for label in labels {
                let entry = found.entry(label.clone()).or_insert_with(|| MissingVerb {
                    label,
                    cards: 0,
                    read_cards: 0,
                    examples: Vec::new(),
                    read_examples: Vec::new(),
                });
                entry.cards += 1;
                entry.read_cards += usize::from(!check.unread);
                if entry.examples.len() < 3 {
                    entry.examples.push(check.name.clone());
                }
                if !check.unread && entry.read_examples.len() < 3 {
                    entry.read_examples.push(check.name.clone());
                }
            }
        }
        let mut verbs: Vec<MissingVerb> = found.into_values().collect();
        verbs.sort_by(|a, b| b.cards.cmp(&a.cards).then_with(|| a.label.cmp(&b.label)));
        verbs
    }

    /// The verbs Forge names for a card whose clauses are all read that its rules lack, by those cards, most first: the
    /// places where the map or the reading of the card is to be checked.
    pub fn missing_in_read_cards(&self) -> Vec<MissingVerb> {
        let mut verbs: Vec<MissingVerb> = self
            .missing_verbs()
            .into_iter()
            .filter(|verb| verb.read_cards > 0)
            .collect();
        verbs.sort_by(|a, b| {
            b.read_cards
                .cmp(&a.read_cards)
                .then_with(|| a.label.cmp(&b.label))
        });
        verbs
    }

    /// The unread tail grouped by Forge API and key.
    pub fn tail(&self, scope: Scope) -> Tail {
        type Key = (Kind, String);
        let mut by_group: BTreeMap<Key, BTreeSet<usize>> = BTreeMap::new();
        let mut keys: BTreeMap<Key, BTreeMap<String, BTreeSet<usize>>> = BTreeMap::new();
        let mut status: BTreeMap<Key, Status> = BTreeMap::new();
        let mut groups_of: Vec<BTreeSet<Key>> = Vec::new();
        for (index, check) in self.checks.iter().enumerate() {
            let mut own = BTreeSet::new();
            if scope.holds(check) {
                // A cost is a part of an ability, read with it.
                for finding in check.gaps().filter(|finding| finding.kind != Kind::Cost) {
                    let group = (finding.kind, finding.name.clone());
                    by_group.entry(group.clone()).or_default().insert(index);
                    let entry = keys.entry(group.clone()).or_default();
                    for key in finding.keys.iter().chain(&finding.key) {
                        entry.entry(key.clone()).or_default().insert(index);
                    }
                    let seen = status.entry(group.clone()).or_insert(finding.status);
                    *seen = (*seen).min(finding.status);
                    own.insert(group);
                }
            }
            groups_of.push(own);
        }
        let mut groups: Vec<Group> = by_group
            .iter()
            .map(|(group, cards)| {
                // A key that nearly every card of a group carries is a parameter of the verb, not a variation of it.
                let own = cards.len();
                let mut group_keys: Vec<(String, usize)> = keys
                    .get(group)
                    .map(|map| {
                        map.iter()
                            .map(|(k, v)| (k.clone(), v.len()))
                            .filter(|(_, n)| own < DEFINING_MIN_CARDS || n * 10 < own * 9)
                            .collect()
                    })
                    .unwrap_or_default();
                group_keys.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                Group {
                    kind: group.0,
                    name: group.1.clone(),
                    status: status.get(group).copied().unwrap_or(Status::Unmapped),
                    cards: cards.len(),
                    alone: cards
                        .iter()
                        .filter(|&&index| groups_of[index].len() == 1)
                        .count(),
                    keys: group_keys,
                }
            })
            .collect();
        groups.sort_by(|a, b| {
            b.cards
                .cmp(&a.cards)
                .then_with(|| (a.kind, &a.name).cmp(&(b.kind, &b.name)))
        });
        let rank: BTreeMap<Key, usize> = groups
            .iter()
            .enumerate()
            .map(|(rank, group)| ((group.kind, group.name.clone()), rank))
            .collect();
        let grouped = groups_of.iter().filter(|own| !own.is_empty()).count();
        let mut covered = Vec::new();
        for readers in READERS.into_iter().filter(|&n| n < groups.len()) {
            let inside = groups_of
                .iter()
                .filter(|own| !own.is_empty() && own.iter().all(|group| rank[group] < readers))
                .count();
            covered.push((readers, inside));
        }
        covered.push((groups.len(), grouped));
        Tail {
            cards: self
                .checks
                .iter()
                .filter(|check| scope.holds(check))
                .count(),
            grouped,
            groups,
            covered,
        }
    }

    /// The report as text: `top` rows for each ranking; with `detail`, a line for every card.
    pub fn render(&self, rows: Rows, detail: bool) -> String {
        let top = rows.top;
        let mut text = String::new();
        let matched = self.checks.len();
        let _ = writeln!(
            text,
            "forge-check: {} stored cards, {matched} with a Forge script ({}), {} without",
            self.stored,
            percent(matched, self.stored),
            self.absent.len()
        );
        self.render_states(&mut text);
        self.render_missing(&mut text, top);
        for scope in Scope::ALL {
            self.render_tail(&mut text, rows, scope);
        }
        if detail {
            self.render_cards(&mut text);
        }
        text.trim_end().to_owned()
    }

    fn render_states(&self, text: &mut String) {
        let matched = self.checks.len();
        let states: Vec<String> = self
            .states()
            .into_iter()
            .map(|(bucket, cards)| format!("{} {cards}", bucket.name()))
            .collect();
        let _ = writeln!(text, "state of those cards: {}", states.join(", "));
        let sources: Vec<String> = [
            (RulesSource::HandWritten, "hand-written"),
            (RulesSource::Compiled, "compiled"),
            (RulesSource::Absent, "none"),
        ]
        .into_iter()
        .map(|(source, word)| {
            let cards = self.checks.iter().filter(|c| c.source == source).count();
            format!("{word} {cards}")
        })
        .collect();
        let _ = writeln!(text, "rules of those cards: {}", sources.join(", "));
        let items = self.items();
        let _ = writeln!(
            text,
            "items their scripts name: {} ({matched} cards): read {} ({}), missing a mapped verb {} ({}), no DSL verb {} ({}), not in the map {} ({})",
            items.all,
            items.read,
            percent(items.read, items.all),
            items.missing,
            percent(items.missing, items.all),
            items.no_verb,
            percent(items.no_verb, items.all),
            items.unmapped,
            percent(items.unmapped, items.all),
        );
    }

    fn render_missing(&self, text: &mut String, top: usize) {
        let missing = self.missing_verbs();
        let cards = self
            .checks
            .iter()
            .filter(|check| check.gaps().any(|f| f.status == Status::Missing))
            .count();
        let _ = writeln!(
            text,
            "\nverbs Forge names that our rules lack ({} verbs, {cards} cards); the second number counts cards whose clauses are all read:",
            missing.len()
        );
        for verb in missing.iter().take(top) {
            let _ = writeln!(
                text,
                "  {:>5} {:>5}  {}  ({})",
                verb.cards,
                verb.read_cards,
                verb.label,
                verb.examples.join(", ")
            );
        }
        let in_read = self.missing_in_read_cards();
        let _ = writeln!(
            text,
            "\nof them, in cards whose clauses are all read ({} verbs; the map or the reading is to be checked):",
            in_read.len()
        );
        for verb in in_read.iter().take(top) {
            let _ = writeln!(
                text,
                "  {:>5}  {}  ({})",
                verb.read_cards,
                verb.label,
                verb.read_examples.join(", ")
            );
        }
    }

    fn render_tail(&self, text: &mut String, rows: Rows, scope: Scope) {
        let tail = self.tail(scope);
        let _ = writeln!(
            text,
            "\nunread tail, {}: {} cards; {} have items we do not read, {} have none",
            scope.title(),
            tail.cards,
            tail.grouped,
            tail.cards - tail.grouped
        );
        let _ = writeln!(
            text,
            "groups by Forge API and key (cards, cards with no other group; map: verb, none = no DSL verb, unmapped):"
        );
        for group in tail.groups.iter().take(rows.top) {
            let keys: Vec<String> = group
                .keys
                .iter()
                .take(rows.keys)
                .map(|(key, cards)| format!("{key} {cards}"))
                .collect();
            let _ = writeln!(
                text,
                "  {:>5} {:>5}  {} [{}]  {}",
                group.cards,
                group.alone,
                group.label(),
                status_word(group.status),
                keys.join(", ")
            );
        }
        let _ = writeln!(
            text,
            "share of the grouped tail one reader per group covers:"
        );
        for (readers, inside) in &tail.covered {
            let _ = writeln!(
                text,
                "  top {readers:>3} groups: {inside} of {} cards ({})",
                tail.grouped,
                percent(*inside, tail.grouped)
            );
        }
    }

    fn render_cards(&self, text: &mut String) {
        let _ = writeln!(
            text,
            "\ncards Forge has, with the items of its script that our rules do not read:"
        );
        for check in &self.checks {
            let gaps: BTreeSet<String> = check.gaps().map(label_of).collect();
            let state = if check.unread { "unread" } else { "read" };
            let source = match check.source {
                RulesSource::HandWritten => "hand-written",
                RulesSource::Compiled => "compiled",
                RulesSource::Draft => "draft",
                RulesSource::Absent => "no rules",
            };
            let _ = writeln!(
                text,
                "  {} [{}, {source}, {state}]: {}",
                check.name,
                Bucket::of(check.support).name(),
                if gaps.is_empty() {
                    "nothing".to_owned()
                } else {
                    gaps.into_iter().collect::<Vec<_>>().join("; ")
                }
            );
            let _ = writeln!(text, "      our verbs: {}", check.ours.join(" "));
        }
        if !self.absent.is_empty() {
            let _ = writeln!(text, "\nstored cards with no Forge script:");
            for name in &self.absent {
                let _ = writeln!(text, "  {name}");
            }
        }
    }
}

#[cfg(test)]
mod tests;

/// The word for how the map stands on a group.
fn status_word(status: Status) -> &'static str {
    match status {
        Status::Read | Status::Missing => "verb",
        Status::NoVerb => "none",
        Status::Unmapped => "unmapped",
    }
}

/// A finding as one label: the verbs that stand for it and the Forge item (with the key of an exception) in brackets.
fn label_of(finding: &Finding) -> String {
    let item = match &finding.key {
        Some(key) => format!("{}+{key}", finding.name),
        None => finding.name.clone(),
    };
    let prefix = if finding.kind == Kind::Effect {
        String::new()
    } else {
        format!("{} ", finding.kind.word())
    };
    if finding.expect.is_empty() {
        format!("{prefix}{item} [{}]", status_word(finding.status))
    } else {
        format!("{} ({prefix}{item})", finding.expect.join(" | "))
    }
}

/// `part` of `whole` as a percentage with one decimal.
fn percent(part: usize, whole: usize) -> String {
    if whole == 0 {
        return "0%".to_owned();
    }
    let tenths = (part * 1000 + whole / 2) / whole;
    format!("{}.{}%", tenths / 10, tenths % 10)
}
