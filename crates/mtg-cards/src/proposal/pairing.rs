//! Pairing: which ability of a proposal stands for which clause of the printed text, and whether the renderers write it back as that clause.

use super::blame;
use super::{ClauseReport, ClauseState, Stray};
use crate::ability::Ability;
use crate::card::{Card, Span};
use crate::compile::{
    Clause, ClauseKind, read_keyword, render_variants, same_text, static_pair_texts,
};
use crate::keyword::Keyword;
use crate::trigger::TriggerPattern;

/// The pairing of a whole card.
#[derive(Debug, Default)]
pub(super) struct Paired {
    /// Every clause of every face in order.
    pub clauses: Vec<ClauseReport>,
    /// Abilities without a clause.
    pub strays: Vec<Stray>,
    /// For each face and each ability as proposed, where the clause it stands for starts.
    pub places: Vec<Vec<Option<u32>>>,
}

impl Paired {
    /// The spans of the clauses of a face that no ability stands for.
    pub(super) fn open(&self, face: usize) -> impl Iterator<Item = Span> + '_ {
        self.clauses
            .iter()
            .filter(move |clause| clause.face == face && clause.state == ClauseState::Open)
            .map(|clause| clause.span)
    }
}

/// Pairs the abilities of every face with the clauses of its text.
pub(super) fn pair_card(card: &Card, clauses: &[Clause]) -> Paired {
    let faces = clauses
        .iter()
        .map(|clause| clause.face + 1)
        .max()
        .unwrap_or(0)
        .max(card.faces.len());
    let mut paired = Paired::default();
    for face in 0..faces {
        let abilities = card
            .faces
            .get(face)
            .map_or(&[][..], |rules| rules.abilities.as_slice());
        let of_face: Vec<&Clause> = clauses.iter().filter(|c| c.face == face).collect();
        let found = Pairer::new(face, abilities, &of_face).run();
        paired.clauses.extend(found.clauses);
        paired.strays.extend(found.strays);
        paired.places.extend(found.places);
    }
    paired
}

/// The clause kinds an ability can stand for.
fn kinds_of(ability: &Ability) -> &'static [ClauseKind] {
    match ability {
        Ability::Spell { .. } => &[ClauseKind::Spell, ClauseKind::Modal],
        Ability::Triggered { .. } => &[ClauseKind::Triggered],
        Ability::Replacement { .. }
        | Ability::Rule { .. }
        | Ability::Static { .. }
        | Ability::AdditionalCost { .. }
        | Ability::FreeCast { .. }
        | Ability::CostModifier { .. } => &[ClauseKind::Static],
        Ability::Loyalty { .. } => &[ClauseKind::Loyalty],
        Ability::Activated { .. } => &[ClauseKind::Activated],
        Ability::Keyword(_) => &[ClauseKind::Keyword],
        Ability::Manual { .. } => &[],
    }
}

/// The clause without its ability word ("Landfall — "), which is flavor, and without the numerals of a Saga chapter.
fn stripped(text: &str) -> &str {
    let Some((word, rest)) = text.split_once(" \u{2014} ") else {
        return text;
    };
    let ability_word = !word.contains(['~', ',', '.', ':']) && word.split(' ').count() <= 3;
    let numerals = word
        .chars()
        .all(|c| matches!(c, 'I' | 'V' | 'X' | ',' | ' '));
    if ability_word || numerals { rest } else { text }
}

/// Whether a clause is a block that reads into several abilities, none of which the renderers write: a level of a
/// leveler (`LEVEL 2-6`), of a Class (`{3}{U}: Level 2`) or of a Station (`8+ | Flying`).
fn is_block(text: &str) -> bool {
    let head = text.lines().next().unwrap_or("");
    let station = head.split_once("+ | ").is_some_and(|(threshold, _)| {
        !threshold.is_empty() && threshold.chars().all(|c| c.is_ascii_digit())
    });
    head.starts_with("LEVEL ") || head.contains(": Level ") || station
}

/// A keyword without the abilities a Backup lists, which the following clauses give it.
fn bare(keyword: &Keyword) -> Keyword {
    match keyword {
        Keyword::Backup { count, .. } => Keyword::Backup {
            count: *count,
            abilities: Vec::new(),
        },
        other => other.clone(),
    }
}

/// The number of leading characters two texts share.
fn common_prefix(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(a, b)| a == b)
        .count()
}

struct Slot<'a> {
    clause: &'a Clause,
    text: &'a str,
    state: Option<ClauseState>,
}

/// Where a leftover ability goes.
enum Choice {
    /// Onto these clauses (the spell lines together, one clause for any other).
    Pair(Vec<usize>),
    /// Into the block of the clause before it.
    Absorb(usize),
    /// Nowhere.
    Stray,
}

struct Pairer<'a> {
    face: usize,
    abilities: &'a [Ability],
    slots: Vec<Slot<'a>>,
    taken: Vec<bool>,
    slot_of: Vec<Option<usize>>,
    places: Vec<Option<u32>>,
    strays: Vec<Stray>,
}

impl<'a> Pairer<'a> {
    fn new(face: usize, abilities: &'a [Ability], clauses: &[&'a Clause]) -> Self {
        Pairer {
            face,
            abilities,
            slots: clauses
                .iter()
                .map(|&clause| Slot {
                    clause,
                    text: stripped(&clause.text),
                    state: None,
                })
                .collect(),
            taken: vec![false; abilities.len()],
            slot_of: vec![None; abilities.len()],
            places: vec![None; abilities.len()],
            strays: Vec::new(),
        }
    }

    fn run(mut self) -> Paired {
        self.manual();
        self.keywords();
        self.static_pairs();
        self.exact();
        self.leftovers();
        let face = self.face;
        Paired {
            clauses: self
                .slots
                .into_iter()
                .map(|slot| ClauseReport {
                    face,
                    span: slot.clause.span,
                    text: slot.clause.text.clone(),
                    kind: slot.clause.kind,
                    state: slot.state.unwrap_or(ClauseState::Open),
                })
                .collect(),
            strays: self.strays,
            places: vec![self.places],
        }
    }

    /// Marks an ability as standing for the clause at `slot`.
    fn settle(&mut self, at: usize, slot: usize, state: ClauseState) {
        self.taken[at] = true;
        self.slot_of[at] = Some(slot);
        self.places[at] = Some(self.slots[slot].clause.span.start);
        self.slots[slot].state = Some(state);
    }

    /// The first clause of one of the kinds whose text a wording equals; `again` also takes one that is read already.
    fn find_text(&self, kinds: &[ClauseKind], written: &[String], again: bool) -> Option<usize> {
        self.slots.iter().position(|slot| {
            let free = slot.state.is_none()
                || (again && matches!(slot.state, Some(ClauseState::Reads { .. })));
            free && kinds.contains(&slot.clause.kind)
                && written.iter().any(|variant| {
                    same_text(variant, slot.text) || same_text(stripped(variant), slot.text)
                })
        })
    }

    /// The clauses a `Manual` ability of the proposal leaves to the table.
    fn manual(&mut self) {
        let abilities = self.abilities;
        for (at, ability) in abilities.iter().enumerate() {
            let Ability::Manual { reason, span } = ability else {
                continue;
            };
            self.taken[at] = true;
            self.places[at] = Some(span.start);
            for slot in &mut self.slots {
                if slot.state.is_none() && slot.clause.span.overlaps(span) {
                    slot.state = Some(ClauseState::Manual {
                        reason: reason.clone(),
                    });
                }
            }
        }
    }

    /// Keyword abilities, read back by the keyword reader.
    fn keywords(&mut self) {
        let abilities = self.abilities;
        for (at, ability) in abilities.iter().enumerate() {
            let Ability::Keyword(keyword) = ability else {
                continue;
            };
            let found = self.slots.iter().position(|slot| {
                slot.state.is_none()
                    && slot.clause.kind == ClauseKind::Keyword
                    && read_keyword(&slot.clause.text).as_ref().map(bare) == Some(bare(keyword))
            });
            if let Some(slot) = found {
                self.settle(at, slot, ClauseState::Reads { ability: at });
            }
        }
    }

    /// Two statics under one condition are one sentence of the card.
    fn static_pairs(&mut self) {
        let abilities = self.abilities;
        for at in 1..abilities.len() {
            if self.taken[at - 1] || self.taken[at] {
                continue;
            }
            let written = static_pair_texts(&abilities[at - 1], &abilities[at]);
            if written.is_empty() {
                continue;
            }
            if let Some(slot) = self.find_text(&[ClauseKind::Static], &written, false) {
                self.settle(at - 1, slot, ClauseState::Reads { ability: at - 1 });
                self.taken[at] = true;
                self.slot_of[at] = Some(slot);
                self.places[at] = self.places[at - 1];
            }
        }
    }

    /// The abilities a renderer writes as a clause, in the clauses of their kind. A line of Saga chapters is read by
    /// each of its chapters.
    fn exact(&mut self) {
        let abilities = self.abilities;
        for (at, ability) in abilities.iter().enumerate() {
            if self.taken[at] {
                continue;
            }
            let written = render_variants(ability);
            if written.is_empty() {
                continue;
            }
            if matches!(ability, Ability::Spell { .. }) {
                let open = self.open_slots(kinds_of(ability));
                let joined = self.joined(&open);
                if written.iter().any(|variant| same_text(variant, &joined)) {
                    for &slot in &open {
                        self.settle(at, slot, ClauseState::Reads { ability: at });
                    }
                }
                continue;
            }
            let chapter = matches!(
                ability,
                Ability::Triggered {
                    when: TriggerPattern::Chapter { .. },
                    ..
                }
            );
            if let Some(slot) = self.find_text(kinds_of(ability), &written, chapter) {
                self.settle(at, slot, ClauseState::Reads { ability: at });
            }
        }
    }

    /// The abilities left take the next clause of their kind, as a mismatch or a construct without a writer.
    fn leftovers(&mut self) {
        let abilities = self.abilities;
        for (at, ability) in abilities.iter().enumerate() {
            if self.taken[at] {
                continue;
            }
            match self.choose(at, ability) {
                Choice::Pair(open) => {
                    let first = open[0];
                    let state = if self.is_block_slot(first) {
                        ClauseState::Unwritten {
                            ability: at,
                            construct: format!(
                                "level, Class or Station block ({})",
                                blame::name(ability)
                            ),
                        }
                    } else {
                        let joined = self.joined(&open);
                        unmatched(ability, at, &joined, &self.slots[first].clause.text)
                    };
                    for &slot in &open {
                        self.settle(at, slot, state.clone());
                    }
                }
                Choice::Absorb(slot) => {
                    self.taken[at] = true;
                    self.slot_of[at] = Some(slot);
                    self.places[at] = Some(self.slots[slot].clause.span.start);
                }
                Choice::Stray => {
                    self.taken[at] = true;
                    self.strays.push(Stray {
                        face: self.face,
                        ability: at,
                        construct: blame::name(ability),
                    });
                }
            }
        }
    }

    /// The clause of its kind that is no block, else a block no ability stands for yet, else the block of the ability
    /// before it, else none.
    fn choose(&self, at: usize, ability: &Ability) -> Choice {
        let own: Vec<usize> = self
            .open_slots(kinds_of(ability))
            .into_iter()
            .filter(|&slot| !self.is_block_slot(slot))
            .collect();
        match (own.first(), matches!(ability, Ability::Spell { .. })) {
            (Some(_), true) => return Choice::Pair(own),
            (Some(&first), false) => return Choice::Pair(vec![first]),
            (None, _) => {}
        }
        if !matches!(ability, Ability::Keyword(_) | Ability::Spell { .. }) {
            let block = (0..self.slots.len())
                .find(|&slot| self.slots[slot].state.is_none() && self.is_block_slot(slot));
            if let Some(slot) = block {
                return Choice::Pair(vec![slot]);
            }
            let before = at
                .checked_sub(1)
                .and_then(|previous| self.slot_of[previous])
                .filter(|&slot| self.is_block_slot(slot));
            if let Some(slot) = before {
                return Choice::Absorb(slot);
            }
        }
        Choice::Stray
    }

    fn is_block_slot(&self, slot: usize) -> bool {
        is_block(&self.slots[slot].clause.text)
    }

    fn open_slots(&self, kinds: &[ClauseKind]) -> Vec<usize> {
        (0..self.slots.len())
            .filter(|&at| {
                self.slots[at].state.is_none() && kinds.contains(&self.slots[at].clause.kind)
            })
            .collect()
    }

    fn joined(&self, slots: &[usize]) -> String {
        slots
            .iter()
            .map(|&at| self.slots[at].text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Why an ability does not read back as its clause: the renderers write other text, or cannot write it.
fn unmatched(ability: &Ability, at: usize, wanted: &str, clause: &str) -> ClauseState {
    if let Ability::Keyword(_) = ability {
        return match read_keyword(clause) {
            Some(other) => ClauseState::Differs {
                ability: at,
                written: format!("{other:?}"),
            },
            None => ClauseState::Unwritten {
                ability: at,
                construct: format!(
                    "{} (the keyword reader does not read this line)",
                    blame::name(ability)
                ),
            },
        };
    }
    let written = render_variants(ability);
    match written
        .into_iter()
        .max_by_key(|variant| common_prefix(variant, wanted))
    {
        Some(written) => ClauseState::Differs {
            ability: at,
            written,
        },
        None => ClauseState::Unwritten {
            ability: at,
            construct: blame::construct(ability),
        },
    }
}
