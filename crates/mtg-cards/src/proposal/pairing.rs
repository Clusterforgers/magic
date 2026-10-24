//! Pairing: which ability of a proposal stands for which clause of the printed text, and whether the renderers write it back as that clause.

use super::blame;
use super::blocks;
use super::{ClauseReport, ClauseState, Stray};
use crate::ability::Ability;
use crate::card::{Card, Span};
use crate::compile::{
    Clause, ClauseKind, block_plan, creature_at, die_row, die_row_texts, own_names, read_keyword,
    read_keywords, same_text, static_pair_texts, written_variants,
};
use crate::keyword::Keyword;
use crate::support::PrintedCard;
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
pub(super) fn pair_card(card: &Card, clauses: &[Clause], printed: &PrintedCard) -> Paired {
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
        let creature_at = printed.faces.get(face).and_then(creature_at);
        let names = printed.faces.get(face).map(own_names).unwrap_or_default();
        let found = Pairer::new(face, abilities, &of_face, (creature_at, names)).run();
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
    let ability_word = !word.contains(['~', ',', '.', ':']) && word.split(' ').count() <= 5;
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

/// Whether a clause is a row of the table of a die roll: a range of totals, then `|` ("1—9 | ...", "15+ | ...", "20 | ...").
fn is_die_row(text: &str) -> bool {
    text.lines().next().and_then(die_row).is_some()
}

/// Whether an ability rolls a die, in any of its effect lists.
fn rolls_dice(ability: &Ability) -> bool {
    serde_json::to_string(ability).is_ok_and(|json| json.contains("\"RollDice\""))
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
    /// Where a Spacecraft with a printed power becomes a creature, for the head of a station block.
    creature_at: Option<i32>,
    /// The names the face calls itself by, which the clauses show as `~` (also inside the name of a token).
    names: Vec<String>,
    slots: Vec<Slot<'a>>,
    taken: Vec<bool>,
    slot_of: Vec<Option<usize>>,
    places: Vec<Option<u32>>,
    strays: Vec<Stray>,
}

impl<'a> Pairer<'a> {
    fn new(
        face: usize,
        abilities: &'a [Ability],
        clauses: &[&'a Clause],
        (creature_at, names): (Option<i32>, Vec<String>),
    ) -> Self {
        Pairer {
            face,
            abilities,
            creature_at,
            names,
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
        self.blocks();
        self.leftovers();
        self.tables();
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
                    self.same(variant, slot.text) || self.same(stripped(variant), slot.text)
                })
        })
    }

    /// Whether a written wording is the clause text; the face's own names count as `~` in it, as in the clause.
    fn same(&self, variant: &str, text: &str) -> bool {
        if same_text(variant, text) {
            return true;
        }
        let mut shown = variant.to_owned();
        for name in &self.names {
            shown = shown.replace(name.as_str(), "~");
        }
        shown != variant && same_text(&shown, text)
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
            let Ability::Keyword(_) = ability else {
                continue;
            };
            if self.taken[at] {
                continue;
            }
            let found = self.slots.iter().position(|slot| {
                slot.state.is_none()
                    && slot.clause.kind == ClauseKind::Keyword
                    && self.run_reads(at, &read_keywords(&slot.clause.text).unwrap_or_default())
            });
            if let Some(slot) = found {
                self.settle(at, slot, ClauseState::Reads { ability: at });
                // A clause that reads into several keywords ("protection from white and from blue") stands for the run.
                let more = read_keywords(&self.slots[slot].clause.text).map_or(0, |all| all.len());
                for later in at + 1..at + more {
                    self.taken[later] = true;
                    self.slot_of[later] = Some(slot);
                    self.places[later] = Some(self.slots[slot].clause.span.start);
                }
            }
        }
    }

    /// Whether the abilities from `at` on are the keywords, one after the other.
    fn run_reads(&self, at: usize, keywords: &[Keyword]) -> bool {
        !keywords.is_empty()
            && keywords.iter().enumerate().all(|(offset, keyword)| {
                matches!(
                    self.abilities.get(at + offset),
                    Some(Ability::Keyword(found)) if bare(found) == bare(keyword)
                ) && (offset == 0 || !self.taken[at + offset])
            })
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
            let written = written_variants(ability);
            if written.is_empty() {
                continue;
            }
            if matches!(ability, Ability::Spell { .. }) {
                let open = self.open_slots(kinds_of(ability));
                let joined = self.joined(&open);
                if written.iter().any(|variant| self.same(variant, &joined)) {
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

    /// The blocks of a leveler, a Class or a Spacecraft: the abilities that work under a block's level stand for it.
    fn blocks(&mut self) {
        for slot in 0..self.slots.len() {
            let text = &self.slots[slot].clause.text;
            if self.slots[slot].state.is_some() || !is_block(text) {
                continue;
            }
            let Some(plan) = block_plan(text, self.creature_at) else {
                continue;
            };
            let free: Vec<usize> = (0..self.abilities.len())
                .filter(|&at| !self.taken[at])
                .collect();
            let Some(found) = blocks::read(&plan, self.abilities, &free) else {
                continue;
            };
            for &at in &found.claimed {
                self.taken[at] = true;
                self.slot_of[at] = Some(slot);
                self.places[at] = Some(self.slots[slot].clause.span.start);
            }
            for &at in &found.strays {
                self.taken[at] = true;
                self.strays.push(Stray {
                    face: self.face,
                    ability: at,
                    construct: blame::name(&self.abilities[at]),
                });
            }
            self.slots[slot].state = Some(found.state);
        }
    }

    /// The rows of the table of a die roll (`1—9 | ...`) stand for the ability of the clause before them when it rolls; a
    /// row is read back when it is the row of the table at its place, written as the line says.
    fn tables(&mut self) {
        for at in 1..self.slots.len() {
            if self.slots[at].state.is_some() || !is_die_row(&self.slots[at].clause.text) {
                continue;
            }
            let rolls = match &self.slots[at - 1].state {
                Some(
                    ClauseState::Reads { ability }
                    | ClauseState::Unwritten { ability, .. }
                    | ClauseState::Differs { ability, .. },
                ) => self.abilities.get(*ability).is_some_and(rolls_dice),
                _ => false,
            };
            if !rolls {
                continue;
            }
            let head = (0..at)
                .rev()
                .find(|&slot| !is_die_row(&self.slots[slot].clause.text))
                .unwrap_or(at - 1);
            let mut state = self.slots[at - 1].state.clone();
            if let (Some(ClauseState::Reads { ability }), Some(rows)) = (
                &self.slots[head].state,
                self.slots[head]
                    .state
                    .as_ref()
                    .and_then(|state| match state {
                        ClauseState::Reads { ability } => self.abilities.get(*ability),
                        _ => None,
                    })
                    .map(die_row_texts),
            ) {
                let text = &self.slots[at].clause.text;
                let wanted = rows.get(at - head - 1).map_or(&[][..], Vec::as_slice);
                if !wanted.iter().any(|written| same_text(written, text)) {
                    let written = wanted
                        .iter()
                        .max_by_key(|variant| common_prefix(variant, text))
                        .cloned()
                        .unwrap_or_default();
                    state = Some(ClauseState::Differs {
                        ability: *ability,
                        written,
                    });
                }
            }
            self.slots[at].state = state;
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
pub(super) fn unmatched(ability: &Ability, at: usize, wanted: &str, clause: &str) -> ClauseState {
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
    let written = written_variants(ability);
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

#[cfg(test)]
mod tests {
    //! Goal: a row of the table of a die roll is told from other clauses by its range of totals and its bar, whatever the range looks like, and only an ability that rolls a die owns the rows after its clause.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::a_range("1—9 | Each player sacrifices a permanent of their choice.", true)]
    #[case::a_single_total("20 | Each opponent sacrifices two permanents.", true)]
    #[case::an_open_range("15+ | Scry X, then draw X cards.", true)]
    #[case::the_first_line_only("1—14 | Draw X cards.\nSecond line", true)]
    #[case::no_bar("1—9 Each player sacrifices a permanent.", false)]
    #[case::words_before_the_bar("Siege Monster — When this creature enters, roll a d20.", false)]
    #[case::no_range(" | Draw a card.", false)]
    #[case::a_name_before_the_bar("Level | 3", false)]
    fn rows_are_told_by_their_range_and_bar(#[case] text: &str, #[case] row: bool) {
        assert_eq!(is_die_row(text), row);
    }

    #[rstest]
    #[case::rolls(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"RollDice":{"sides":4,"results":[{"from":1,"effects":[{"Draw":{}}]}]}}]}}"#, true)]
    #[case::inside_a_repeat(r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"p","effects":[{"RollDice":{"sides":6,"results":[{"from":1,"effects":[{"Draw":{}}]}]}}]}}]}}"#, true)]
    #[case::does_not_roll(r#"{"Spell":{"effects":[{"FlipCoin":{"win":[{"Draw":{}}]}}]}}"#, false)]
    fn an_ability_that_rolls_owns_the_rows(#[case] json: &str, #[case] rolls: bool) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        assert_eq!(rolls_dice(&ability), rolls);
    }
}
