//! Coverage: how well the compiler and the engine cover a set of cards, and which work would free the most of them.
//!
//! [`analyze`] gives every card its rules (the hand-written ones, else the compiler's), its [`Support`] for the
//! engine's [`Caps`] and its [`Blocker`]s; the [`Report`] ranks the blockers by the cards they hold back
//! (`docs/cards-dsl.md`, Compiler and Manual fallback).

#[cfg(test)]
mod draft_tests;
mod render;
mod shape;
#[cfg(test)]
mod tests;

pub use render::DEFAULT_TOP;

use std::collections::{BTreeMap, BTreeSet};

use crate::ability::Ability;
use crate::assemble::printed_gaps;
use crate::builtin;
use crate::card::{Card, ImplStatus};
use crate::compile::{Compiled, compile};
use crate::draft::ReadState;
use crate::load::{Library, Source, canonical, slug};
use crate::support::{Caps, PrintedCard, Support, support};
use crate::validate::{check_printed, used_primitives};

/// What the report says of a printed gap that lies on a face the engine does not play.
const FACES_GAP: &str =
    "you cast or turn over the other face by hand: the game plays only the front face of a card";

/// A card of the analysed decks, with the copies each deck holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreconCard {
    /// The imported card.
    pub printed: PrintedCard,
    /// Copies in each deck, by the deck's label.
    pub copies: BTreeMap<String, u32>,
}

/// Where a card's rules came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulesSource {
    /// A hand-written file of the embedded library.
    HandWritten,
    /// The compiler's reading of the oracle text.
    Compiled,
    /// A draft a player read and found correct (Read tier).
    Draft,
    /// None: the card has no rules text, or the data cannot hold it.
    Absent,
}

/// One thing that keeps a card from being `Verified`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Blocker {
    /// A clause the compiler leaves to the table, as its kind and shape (`triggered: When ~ dies, N`).
    Clause(String),
    /// A primitive of the card's rules that the engine does not run (`Effect.Mill`).
    Primitive(String),
    /// A printed characteristic the engine cannot play with (an `{X}` cost, a back face).
    Printed(String),
    /// The rules cannot be used at all: `invalid`, `stale`, `unplayable`, `no rules`.
    Rules(String),
}

impl Blocker {
    /// The blocker as one line of text.
    pub fn label(&self) -> String {
        match self {
            Blocker::Clause(shape) => shape.clone(),
            Blocker::Primitive(name) => format!("engine: {name}"),
            Blocker::Printed(what) => format!("printed: {what}"),
            Blocker::Rules(what) => format!("rules: {what}"),
        }
    }
}

/// One analysed card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardResult {
    /// The oracle id.
    pub oracle_id: String,
    /// The card's name.
    pub name: String,
    /// Where its rules came from.
    pub rules: RulesSource,
    /// How well the game plays it.
    pub support: Support,
    /// What holds it back; empty exactly when it is `Verified` or `Read`.
    pub blockers: BTreeSet<Blocker>,
    /// Whether a draft of the card waits for its first read: it is pending, or stale since the text or the draft changed,
    /// and no rules file or `Verified` reading outranks it.
    pub waiting: bool,
    /// Copies in each deck, by the deck's label.
    pub copies: BTreeMap<String, u32>,
}

impl CardResult {
    /// The copies in all decks.
    pub fn total_copies(&self) -> u32 {
        self.copies.values().sum()
    }

    /// The bucket the report counts the card in: `Waiting` for a card with a draft to read, else that of its state.
    pub fn bucket(&self) -> Bucket {
        if self.waiting {
            Bucket::Waiting
        } else {
            Bucket::of(self.support)
        }
    }
}

/// The states the report groups cards by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    /// `Verified`.
    Verified,
    /// `Read`: a draft a player read and found correct.
    Read,
    /// A card whose draft waits for its first read; it plays by hand (`Partial` or `Manual`) until then.
    Waiting,
    /// `Partial`, with no draft waiting.
    Partial,
    /// `Manual`, with no draft waiting.
    Manual,
    /// `Stale` and `Unplayable`.
    Other,
}

impl Bucket {
    /// Every bucket in report order.
    pub const ALL: [Bucket; 6] = [
        Bucket::Verified,
        Bucket::Read,
        Bucket::Waiting,
        Bucket::Partial,
        Bucket::Manual,
        Bucket::Other,
    ];

    /// The bucket of a state alone; it is never `Waiting`, which needs the card's draft ([`CardResult::bucket`]).
    pub const fn of(state: Support) -> Bucket {
        match state {
            Support::Verified => Bucket::Verified,
            Support::Read => Bucket::Read,
            Support::Partial => Bucket::Partial,
            Support::Manual => Bucket::Manual,
            Support::Stale | Support::Unplayable => Bucket::Other,
        }
    }

    /// The bucket's name in the report.
    pub const fn name(self) -> &'static str {
        match self {
            Bucket::Verified => "verified",
            Bucket::Read => "read",
            Bucket::Waiting => "waiting",
            Bucket::Partial => "partial",
            Bucket::Manual => "manual",
            Bucket::Other => "other",
        }
    }
}

/// How many cards, and how many copies of them, are in a bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateTally {
    /// The bucket.
    pub bucket: Bucket,
    /// Distinct cards.
    pub cards: u32,
    /// Copies across all decks.
    pub copies: u32,
}

/// How much of one deck is `Verified`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckShare {
    /// The deck's label.
    pub deck: String,
    /// Copies of all the deck's cards.
    pub copies: u32,
    /// Copies of its `Verified` cards.
    pub verified: u32,
}

impl DeckShare {
    /// The share of the deck's copies that are `Verified`, from 0 to 1.
    pub fn share(&self) -> f64 {
        if self.copies == 0 {
            0.0
        } else {
            f64::from(self.verified) / f64::from(self.copies)
        }
    }
}

/// How many cards one blocker holds back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockerCount {
    /// The blocker.
    pub blocker: Blocker,
    /// Cards it alone holds back: those whose blockers are exactly this one.
    pub alone: u32,
    /// Cards it holds back, with or without others.
    pub in_all: u32,
}

/// Every analysed card, by name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// The cards in name order.
    pub cards: Vec<CardResult>,
}

impl Report {
    /// Distinct cards and copies per bucket, in report order.
    pub fn tally(&self) -> Vec<StateTally> {
        Bucket::ALL
            .into_iter()
            .map(|bucket| {
                let of_bucket = self.cards.iter().filter(|card| card.bucket() == bucket);
                let (cards, copies) = of_bucket.fold((0, 0), |(cards, copies), card| {
                    (cards + 1, copies + card.total_copies())
                });
                StateTally {
                    bucket,
                    cards,
                    copies,
                }
            })
            .collect()
    }

    /// The decks by the share of their copies that are `Verified`, the highest first, ties by label.
    pub fn decks(&self) -> Vec<DeckShare> {
        let mut decks: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
        for card in &self.cards {
            for (deck, &copies) in &card.copies {
                let entry = decks.entry(deck).or_default();
                entry.0 += copies;
                if card.support == Support::Verified {
                    entry.1 += copies;
                }
            }
        }
        let mut shares: Vec<DeckShare> = decks
            .into_iter()
            .filter(|&(_, (copies, _))| copies > 0)
            .map(|(deck, (copies, verified))| DeckShare {
                deck: deck.to_owned(),
                copies,
                verified,
            })
            .collect();
        // Compared as fractions of integers, so equal shares tie exactly.
        shares.sort_by(|a, b| {
            let left = u64::from(a.verified) * u64::from(b.copies);
            let right = u64::from(b.verified) * u64::from(a.copies);
            right.cmp(&left).then_with(|| a.deck.cmp(&b.deck))
        });
        shares
    }

    /// Every blocker with the cards it holds back, ordered by the cards it holds back in all, then
    /// by the cards it holds back alone, then by name.
    pub fn blocker_counts(&self) -> Vec<BlockerCount> {
        let mut counts: BTreeMap<&Blocker, (u32, u32)> = BTreeMap::new();
        for card in &self.cards {
            for blocker in &card.blockers {
                let entry = counts.entry(blocker).or_default();
                entry.1 += 1;
                if card.blockers.len() == 1 {
                    entry.0 += 1;
                }
            }
        }
        let mut counts: Vec<BlockerCount> = counts
            .into_iter()
            .map(|(blocker, (alone, in_all))| BlockerCount {
                blocker: blocker.clone(),
                alone,
                in_all,
            })
            .collect();
        counts.sort_by(|a, b| {
            b.in_all
                .cmp(&a.in_all)
                .then(b.alone.cmp(&a.alone))
                .then_with(|| a.blocker.cmp(&b.blocker))
        });
        counts
    }

    /// The `n` blockers that alone hold back the most cards (ties: the most in all, then by name).
    pub fn top_alone(&self, n: usize) -> Vec<BlockerCount> {
        let mut counts: Vec<BlockerCount> = self
            .blocker_counts()
            .into_iter()
            .filter(|count| count.alone > 0)
            .collect();
        // The stable sort keeps the order of `blocker_counts` among equals.
        counts.sort_by(|a, b| b.alone.cmp(&a.alone).then(b.in_all.cmp(&a.in_all)));
        counts.truncate(n);
        counts
    }

    /// The `n` blockers that hold back the most cards in all (ties: the most alone, then by name).
    pub fn top_all(&self, n: usize) -> Vec<BlockerCount> {
        let mut counts = self.blocker_counts();
        counts.truncate(n);
        counts
    }
}

/// Reads every card through the embedded hand-written rules, else the compiler's, and judges it for `caps`; a draft the
/// library holds counts after them (`Read` once approved, otherwise the card waits for a read).
///
/// Cards with the same oracle id are one card, with their copies added up.
pub fn analyze(cards: &[PreconCard], caps: &Caps) -> Report {
    match builtin::library() {
        Ok(library) => analyze_with(cards, library, caps),
        // The tests of this crate load the embedded rules; without them every card goes to the compiler.
        Err(_) => analyze_with(cards, &Library::default(), caps),
    }
}

/// What a judgment reads besides the card: the engine, the hand-written rules and what compiled
/// rules are validated against.
struct Context<'a> {
    hand_written: &'a Library,
    caps: &'a Caps,
    /// The names of the analyzed cards and their faces, lowercase: what a rule may name (`Partner with`).
    names: BTreeSet<String>,
    /// The predefined tokens of the hand-written library, as files.
    tokens: Vec<Source>,
}

/// [`analyze`] with the hand-written rules given.
pub fn analyze_with(cards: &[PreconCard], hand_written: &Library, caps: &Caps) -> Report {
    let mut distinct: BTreeMap<&str, (&PrintedCard, BTreeMap<String, u32>)> = BTreeMap::new();
    for card in cards {
        let entry = distinct
            .entry(&card.printed.oracle_id)
            .or_insert_with(|| (&card.printed, BTreeMap::new()));
        for (deck, copies) in &card.copies {
            *entry.1.entry(deck.clone()).or_default() += copies;
        }
    }
    let context = Context {
        hand_written,
        caps,
        names: distinct
            .values()
            .flat_map(|(printed, _)| {
                std::iter::once(&printed.name).chain(printed.faces.iter().map(|face| &face.name))
            })
            .map(|name| name.to_lowercase())
            .collect(),
        tokens: hand_written
            .tokens()
            .map(|token| Source {
                path: format!("cards/tokens/{}.json", token.id),
                text: canonical(token),
            })
            .collect(),
    };
    let mut results: Vec<CardResult> = distinct
        .into_values()
        .map(|(printed, copies)| judge(printed, copies, &context))
        .collect();
    results.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.oracle_id.cmp(&b.oracle_id))
    });
    Report { cards: results }
}

/// One card judged: its rules, then its draft, which counts when no rules file or `Verified` reading outranks it
/// ([`Library::support`] has the precedence).
fn judge(
    printed: &PrintedCard,
    copies: BTreeMap<String, u32>,
    context: &Context<'_>,
) -> CardResult {
    let (mut rules, mut state, mut blockers) = judge_rules(printed, context);
    let hand_written = context.hand_written;
    let id = printed.oracle_id.as_str();
    let rules_file = hand_written
        .file_of(id)
        .is_some_and(|file| !file.starts_with("card_rules/"));
    let mut waiting = false;
    if state != Support::Verified
        && !rules_file
        && let Some(draft) = hand_written.draft(id)
    {
        if let Some(read) = draft.record(printed, context.caps) {
            (rules, state, blockers) = (RulesSource::Draft, read.support, BTreeSet::new());
        } else {
            waiting = draft.state_for(printed) == ReadState::Pending;
        }
    }
    CardResult {
        oracle_id: printed.oracle_id.clone(),
        name: printed.name.clone(),
        rules,
        support: state,
        blockers,
        waiting,
        copies,
    }
}

/// The rules that decide a card without its draft (a rules file, else the compiler's reading), the state they earn and
/// what holds it back.
fn judge_rules(
    printed: &PrintedCard,
    context: &Context<'_>,
) -> (RulesSource, Support, BTreeSet<Blocker>) {
    let (hand_written, caps) = (context.hand_written, context.caps);
    let compiled = compile(printed);
    if let Some(card) = hand_written.card(&printed.oracle_id) {
        let state = hand_written.support(printed, caps).support;
        let blockers = blockers_of(Some(card), state, printed, &compiled, caps);
        (RulesSource::HandWritten, state, blockers)
    } else if let Some(card) = &compiled.card {
        match checked_library(card, printed, context) {
            Some(library) => {
                let state = library.support(printed, caps).support;
                let blockers = blockers_of(Some(card), state, printed, &compiled, caps);
                (RulesSource::Compiled, state, blockers)
            }
            None => (
                RulesSource::Compiled,
                Support::Manual,
                BTreeSet::from([Blocker::Rules("invalid".to_owned())]),
            ),
        }
    } else {
        let state = support(None, printed, caps).support;
        let blockers = blockers_of(None, state, printed, &compiled, caps);
        (RulesSource::Absent, state, blockers)
    }
}

/// The library of one compiled card next to the predefined tokens, as the server would load its
/// stored rules; `None` when the rules do not validate or do not match the imported card.
fn checked_library(card: &Card, printed: &PrintedCard, context: &Context<'_>) -> Option<Library> {
    let mut sources = context.tokens.clone();
    sources.push(Source {
        path: format!("cards/{}.json", slug(&card.name)),
        text: canonical(card),
    });
    let library = Library::from_sources(&sources).ok()?;
    let tokens: BTreeSet<String> = library.tokens().map(|token| token.id.clone()).collect();
    check_printed(card, printed, &context.names, &tokens)
        .is_empty()
        .then_some(library)
}

/// What keeps a card of state `state` from `Verified`: its unread clauses, the primitives of its
/// rules the engine lacks and its printed gaps. A card that is not `Verified` always has one.
fn blockers_of(
    card: Option<&Card>,
    state: Support,
    printed: &PrintedCard,
    compiled: &Compiled,
    caps: &Caps,
) -> BTreeSet<Blocker> {
    let rules = |what: &str| BTreeSet::from([Blocker::Rules(what.to_owned())]);
    match state {
        Support::Verified | Support::Read => return BTreeSet::new(),
        Support::Stale => return rules("stale"),
        Support::Unplayable => return rules("unplayable"),
        Support::Partial | Support::Manual => {}
    }
    let mut found = BTreeSet::new();
    match card {
        Some(card) if card.impl_status != ImplStatus::Manual => {
            for (face, ability) in card.abilities() {
                if let Ability::Manual { span, reason } = ability {
                    let listed = compiled
                        .clauses
                        .iter()
                        .find(|clause| clause.face == face && clause.span.overlaps(span));
                    if listed.is_some_and(|clause| clause.held_by_others) {
                        continue;
                    }
                    found.insert(Blocker::Clause(listed.map_or_else(
                        || format!("manual: {reason}"),
                        |clause| shape::shape(clause.kind, &clause.text),
                    )));
                }
            }
            for (_, ability) in card.abilities() {
                if !ability.is_manual() {
                    found.extend(
                        caps.missing(&used_primitives(ability))
                            .into_iter()
                            .map(Blocker::Primitive),
                    );
                }
            }
        }
        // Rules that read nothing leave every clause of the text to the table.
        _ => found.extend(
            compiled
                .clauses
                .iter()
                .filter(|clause| !clause.held_by_others)
                .map(|clause| Blocker::Clause(shape::shape(clause.kind, &clause.text))),
        ),
    }
    found.extend(printed_gaps(printed, caps).into_iter().map(|gap| {
        Blocker::Printed(if gap.face == 0 {
            gap.reason
        } else {
            FACES_GAP.to_owned()
        })
    }));
    if found.is_empty() {
        return rules("no rules");
    }
    found
}
