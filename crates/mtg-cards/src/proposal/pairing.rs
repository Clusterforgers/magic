//! Pairing: which ability of a proposal stands for which clause of the printed text, and whether the renderers write it back as that clause.

use super::blame;
use super::blocks;
use super::{ClauseReport, ClauseState, Stray};
use crate::ability::Ability;
use crate::card::{Card, Span};
use crate::compile::{
    Clause, ClauseKind, block_plan, creature_at, die_row, die_row_texts, either_way_pair_texts,
    ep23_three_texts, own_names, read_keyword, read_keywords, same_written_text, static_pair_texts,
    written_variants,
};
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::support::PrintedCard;
use crate::trigger::TriggerPattern;

/// The pairing of a whole card.
#[derive(Debug, Default)]
pub(crate) struct Paired {
    /// Every clause of every face in order.
    pub(crate) clauses: Vec<ClauseReport>,
    /// Abilities without a clause.
    pub(crate) strays: Vec<Stray>,
    /// For each face and each ability as proposed, where the clause it stands for starts.
    pub(crate) places: Vec<Vec<Option<u32>>>,
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
pub(crate) fn pair_card(card: &Card, clauses: &[Clause], printed: &PrintedCard) -> Paired {
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
        let attachment = printed
            .faces
            .get(face)
            .map_or(("", String::new()), |printed| {
                (
                    attachment_word(&printed.type_line),
                    enchanted_noun(&printed.oracle_text),
                )
            });
        let cleaved = printed
            .faces
            .get(face)
            .map(crate::compile::cleaved_lines)
            .unwrap_or_default();
        let mut pairing = Pairer::new(face, abilities, &of_face, (creature_at, names, attachment));
        pairing.cleaved = cleaved;
        let found = pairing.run();
        paired.clauses.extend(found.clauses);
        paired.strays.extend(found.strays);
        paired.places.extend(found.places);
    }
    paired
}

/// "equipped" for an Equipment, "enchanted" for an Aura: the word its attached object is named by.
fn attachment_word(type_line: &str) -> &'static str {
    if type_line.contains("Equipment") {
        "equipped"
    } else if type_line.contains("Aura") {
        "enchanted"
    } else {
        ""
    }
}

/// What an Aura enchants by its first line ("Enchant land" is "land"); "creature" for any other line.
fn enchanted_noun(oracle: &str) -> String {
    oracle
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("Enchant "))
        .filter(|noun| !noun.contains(' '))
        .unwrap_or("creature")
        .to_owned()
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
        | Ability::AltCast { .. }
        | Ability::FreeActivation { .. }
        | Ability::InstantActivation { .. }
        | Ability::CastOnly { .. }
        | Ability::OpeningHand
        | Ability::OpeningHandWith { .. }
        | Ability::Sticker(_)
        | Ability::CostModifier { .. } => &[ClauseKind::Static],
        // On an instant or sorcery the reveal is a sentence of the spell's text (Providence).
        Ability::OpeningReveal { .. } => &[ClauseKind::Static, ClauseKind::Spell],
        Ability::Loyalty { .. } => &[ClauseKind::Loyalty],
        Ability::Activated { .. } => &[ClauseKind::Activated],
        // Exert is a sentence of the card ("You may exert this creature as it attacks."), read by the static templates.
        // So is the flash permission whose permanent is sacrificed at the next cleanup step.
        Ability::Keyword(
            Keyword::Exert { .. } | Keyword::FlashSacrifice | Keyword::MacroX { .. },
        ) => &[ClauseKind::Keyword, ClauseKind::Static],
        Ability::Keyword(_) => &[ClauseKind::Keyword],
        Ability::Manual { .. } => &[],
    }
}

/// The clause without its ability word ("Landfall — "), which is flavor, and without the numerals of a Saga chapter;
/// the name of a mode on a later line ("Choose one —\n• Disarm — ...") is no ability word.
fn stripped(text: &str) -> &str {
    let Some((word, rest)) = text.split_once(" \u{2014} ") else {
        return text;
    };
    // "Max speed" is no flavor: it is the condition of the ability (CR 702.179).
    if word == "Max speed" {
        return text;
    }
    // A longer name is flavor as the compiler takes it ("A Thousand Souls Die Every Day — ").
    let ability_word = (!word.contains(['~', ',', '.', ':', '\n']) && word.split(' ').count() <= 5)
        || (!word.contains('~')
            && crate::compile::classify::ability_word_len(text)
                == Some(word.len() + " \u{2014} ".len()));
    let numerals = word
        .chars()
        .all(|c| matches!(c, 'I' | 'V' | 'X' | ',' | ' '));
    if ability_word || numerals { rest } else { text }
}

/// The clause without the ability words that open a later sentence ("Draw three cards. Adamant — If ..."), which are
/// flavor too (CR 207.2c); `None` when it has none (r7c).
fn r7c_unworded(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut found = false;
    // A sentence ends at ". ", or at ".\n" between the clauses of one ability.
    while let Some(at) = [rest.find(". "), rest.find(".\n")]
        .into_iter()
        .flatten()
        .min()
    {
        let (head, after) = rest.split_at(at + 2);
        out.push_str(head);
        rest = after;
        let Some((word, tail)) = rest.split_once(" \u{2014} ") else {
            continue;
        };
        let capital = word.chars().next().is_some_and(char::is_uppercase);
        if capital
            && !word.contains(['~', ',', '.', ':', '\n', '"'])
            && word.split(' ').count() <= 3
        {
            rest = tail;
            found = true;
        }
    }
    out.push_str(rest);
    found.then_some(out)
}

/// Whether a clause is a block that reads into several abilities, none of which the renderers write: a level of a
/// leveler (`LEVEL 2-6`), of a Class (`{3}{U}: Level 2`) or of a Station (`8+ | Flying`).
fn is_block(text: &str) -> bool {
    let head = text.lines().next().unwrap_or("");
    let station = head.split_once("+ | ").is_some_and(|(threshold, _)| {
        !threshold.is_empty() && threshold.chars().all(|c| c.is_ascii_digit())
    });
    head.starts_with("LEVEL ")
        || head.contains(": Level ")
        || station
        || crate::compile::word_block(text).is_some()
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
        // The spell of cleave is read from the spell lines without the bracketed words, not from the keyword line.
        Keyword::Cleave { cost, .. } => Keyword::Cleave {
            cost: cost.clone(),
            spell: Box::new(Ability::Spell {
                targets: Vec::new(),
                effects: Vec::new(),
            }),
        },
        other => super::r6c_bare::canonical(other),
    }
}

const GENDERED: [&str; 8] = [
    "he", "she", "him", "her", "his", "hers", "himself", "herself",
];

/// Whether a text names a person with a gendered pronoun.
fn gendered(text: &str) -> bool {
    text.split(|c: char| !c.is_alphabetic())
        .any(|word| GENDERED.contains(&word.to_lowercase().as_str()))
}

/// The variant sharing the longest prefix with the clause; a gendered wording is a last resort unless the clause is gendered.
fn closest(variants: Vec<String>, wanted: &str) -> Option<String> {
    let person = gendered(wanted);
    variants
        .into_iter()
        .max_by_key(|variant| (person || !gendered(variant), common_prefix(variant, wanted)))
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
    /// "a Dragon was beheld" or "evidence was collected": the words for the paid additional cost of the face.
    beheld: Option<String>,
    /// What the face attaches by: "equipped" for an Equipment, "enchanted" for an Aura, nothing for another card; and
    /// the noun of what an Aura enchants ("land" for "Enchant land").
    attachment: (&'static str, String),
    /// The spell lines of a cleave face without the words in square brackets.
    cleaved: Vec<String>,
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
        (creature_at, names, attachment): (Option<i32>, Vec<String>, (&'static str, String)),
    ) -> Self {
        Pairer {
            face,
            abilities,
            creature_at,
            names,
            beheld: super::e15c_beheld::paid_words(abilities),
            attachment,
            cleaved: Vec::new(),
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
        self.rmt0_cleave();
        self.keywords();
        self.alt_keywords();
        self.equip_options();
        self.champions();
        self.r5g_hand_keywords();
        self.static_pairs();
        self.either_way_pairs();
        self.haunt_pairs();
        self.shared_pairs();
        self.exact();
        self.blocks();
        self.leftovers();
        self.tables();
        self.r5g_escalate();
        self.tiered();
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

    /// The wording with the attached object named by the face's own attachment: the data keeps "what the source is
    /// attached to" and the face says whether that is equipped or enchanted, and what an Aura enchants.
    fn attached(&self, variant: &str) -> String {
        let (word, noun) = &self.attachment;
        let enchanted = format!("nchanted {noun}");
        let (from, to) = match *word {
            "equipped" => ("nchanted creature", "quipped creature"),
            "enchanted" => ("quipped creature", enchanted.as_str()),
            _ => return variant.to_owned(),
        };
        // A quoted ability belongs to the object that gains it, which names its own attachment ("enchanted permanent
        // ... has "Equipped creature has lifelink."").
        let outside_quotes = |text: &str, from: &str, to: &str| {
            text.split('"')
                .enumerate()
                .map(|(at, part)| {
                    if at % 2 == 0 {
                        part.replace(from, to)
                    } else {
                        part.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join("\"")
        };
        let variant = outside_quotes(variant, from, to);
        if *word == "enchanted" && noun != "creature" {
            return outside_quotes(&variant, "nchanted creature", &enchanted);
        }
        variant
    }

    /// Whether a written wording is the clause text; the face's own names count as `~` in it, as in the clause.
    fn same(&self, variant: &str, text: &str) -> bool {
        let worded = self.attached(variant);
        let variant = worded.as_str();
        if same_written_text(variant, text) {
            return true;
        }
        if let Some(unworded) = r7c_unworded(text) {
            return self.same(variant, &unworded);
        }
        if let Some(plain) = self
            .beheld
            .as_deref()
            .and_then(|words| super::e15c_beheld::unpaid_words(text, words))
        {
            return self.same(variant, &plain);
        }
        let mut shown = variant.to_owned();
        for name in &self.names {
            shown = shown.replace(name.as_str(), "~");
        }
        shown != variant && same_written_text(&shown, text)
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

    /// The alternative-cost keywords (warp, surge, web-slinging, impending): an `AltCast` and its riders stand for the line.
    fn alt_keywords(&mut self) {
        let abilities = self.abilities;
        for at in 0..abilities.len() {
            if self.taken[at] {
                continue;
            }
            let Some((line, count)) = crate::alt_riders::keyword_line(&abilities[at..]) else {
                continue;
            };
            if (at..at + count).any(|each| self.taken[each]) {
                continue;
            }
            let Some(slot) = self.find_text(&[ClauseKind::Keyword], &[line], false) else {
                continue;
            };
            self.settle(at, slot, ClauseState::Reads { ability: at });
            for later in at + 1..at + count {
                self.taken[later] = true;
                self.slot_of[later] = Some(slot);
                self.places[later] = Some(self.slots[slot].clause.span.start);
            }
        }
    }

    /// Champion: the trigger that exiles a creature or sacrifices the champion stands for the keyword line.
    fn champions(&mut self) {
        let abilities = self.abilities;
        for (at, ability) in abilities.iter().enumerate() {
            if self.taken[at] {
                continue;
            }
            let Some(line) = crate::compile::champion_line(ability) else {
                continue;
            };
            if let Some(slot) =
                self.find_text(&[ClauseKind::Keyword, ClauseKind::Static], &[line], false)
            {
                self.settle(at, slot, ClauseState::Reads { ability: at });
            }
        }
    }

    /// Reinforce and Forecast: the activated ability from the hand that the keyword means stands for its line.
    fn r5g_hand_keywords(&mut self) {
        let abilities = self.abilities;
        for (at, ability) in abilities.iter().enumerate() {
            if self.taken[at] {
                continue;
            }
            let mut lines = crate::compile::forecast_lines(ability, written_variants);
            lines.extend(crate::compile::reinforce_line(ability));
            if lines.is_empty() {
                continue;
            }
            let kinds = [ClauseKind::Keyword, ClauseKind::Activated];
            if let Some(slot) = self.find_text(&kinds, &lines, false) {
                self.settle(at, slot, ClauseState::Reads { ability: at });
            }
        }
    }

    /// Escalate: the keyword line of a modal spell reads as the cost it adds for each mode beyond the first, beside the
    /// clause the spell stands for.
    fn r5g_escalate(&mut self) {
        for (at, ability) in self.abilities.iter().enumerate() {
            let (Some(cost), Some(_)) = (crate::compile::escalate_cost(ability), self.slot_of[at])
            else {
                continue;
            };
            let found = self.slots.iter().position(|slot| {
                slot.state.is_none()
                    && slot.clause.kind == ClauseKind::Keyword
                    && crate::compile::escalate_reads(slot.text, cost)
            });
            if let Some(slot) = found {
                self.slots[slot].state = Some(ClauseState::Reads { ability: at });
            }
        }
    }

    /// Tiered (CR 702.183): the spell that read its modes, each with a cost of its own and one chosen, stands for the
    /// keyword line.
    fn tiered(&mut self) {
        let Some(at) = (0..self.abilities.len())
            .find(|&at| self.slot_of[at].is_some() && tiered_spell(&self.abilities[at]))
        else {
            return;
        };
        if let Some(slot) = self
            .slots
            .iter()
            .position(|slot| slot.state.is_none() && slot.clause.text.trim() == "Tiered")
        {
            self.slots[slot].state = Some(ClauseState::Reads { ability: at });
        }
    }

    /// Equip with an option: the activated abilities that attach the Equipment stand for the one line of the text.
    fn equip_options(&mut self) {
        let abilities = self.abilities;
        for at in 0..abilities.len() {
            if self.taken[at] {
                continue;
            }
            let Some((line, count)) = crate::compile::equip_line(&abilities[at..]) else {
                continue;
            };
            if (at..at + count).any(|each| self.taken[each]) {
                continue;
            }
            let kinds = [
                ClauseKind::Keyword,
                ClauseKind::Static,
                ClauseKind::Activated,
            ];
            let Some(slot) = self.find_text(&kinds, &[line], false) else {
                continue;
            };
            self.settle(at, slot, ClauseState::Reads { ability: at });
            for later in at + 1..at + count {
                self.taken[later] = true;
                self.slot_of[later] = Some(slot);
                self.places[later] = Some(self.slots[slot].clause.span.start);
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

    /// A haunt creature's "When ~ enters or the creature it haunts dies, ..." is one sentence of two abilities: the
    /// enters trigger and the same trigger from exile when the haunted creature dies (CR 702.55).
    fn haunt_pairs(&mut self) {
        let abilities = self.abilities;
        for at in 1..abilities.len() {
            if self.taken[at - 1] || self.taken[at] {
                continue;
            }
            let written = haunt_pair_texts(&abilities[at - 1], &abilities[at]);
            if written.is_empty() {
                continue;
            }
            if let Some(slot) = self.find_text(&[ClauseKind::Triggered], &written, false) {
                self.settle(at - 1, slot, ClauseState::Reads { ability: at - 1 });
                self.taken[at] = true;
                self.slot_of[at] = Some(slot);
                self.places[at] = self.places[at - 1];
            }
        }
    }

    /// Two different abilities next to each other that write the same sentence are that one sentence of the card: an
    /// eminence trigger and the same trigger from the command zone, "You may look at the top card of your library and at
    /// face-down creatures you don't control any time."
    fn shared_pairs(&mut self) {
        let abilities = self.abilities;
        for at in 1..abilities.len() {
            if self.taken[at - 1] || self.taken[at] {
                continue;
            }
            let written = shared_texts(&abilities[at - 1], &abilities[at]);
            if written.is_empty() {
                continue;
            }
            if let Some(slot) = self.find_text(kinds_of(&abilities[at - 1]), &written, false) {
                self.settle(at - 1, slot, ClauseState::Reads { ability: at - 1 });
                self.taken[at] = true;
                self.slot_of[at] = Some(slot);
                self.places[at] = self.places[at - 1];
            }
        }
    }

    /// Cleave: the keyword stands for its line when its cleaved spell writes the face's spell text without the words in
    /// square brackets, and differs from it otherwise.
    fn rmt0_cleave(&mut self) {
        let abilities = self.abilities;
        let Some((at, (line, cleaved))) = abilities
            .iter()
            .enumerate()
            .find_map(|(at, ability)| super::rmt0_cleave::cleave(ability).map(|found| (at, found)))
        else {
            return;
        };
        let Some(slot) = self.find_text(&[ClauseKind::Keyword], &[line], false) else {
            return;
        };
        let text = self.cleaved.join(" ");
        let written = crate::compile::written_variants(&cleaved);
        if !text.is_empty()
            && written
                .iter()
                .any(|variant| self.same(variant, stripped(&text)))
        {
            self.settle(at, slot, ClauseState::Reads { ability: at });
        } else {
            let wanted = written.first().cloned().unwrap_or_default();
            let state = ClauseState::Differs {
                ability: at,
                written: wanted,
            };
            self.settle(at, slot, state);
        }
    }

    /// Two triggers or two shields for either side of a fight are one sentence of the card ("blocks or becomes blocked
    /// by a creature", "dealt to and dealt by enchanted creature").
    fn either_way_pairs(&mut self) {
        let abilities = self.abilities;
        for at in 1..abilities.len() {
            if self.taken[at - 1] || self.taken[at] {
                continue;
            }
            let written = either_way_pair_texts(&abilities[at - 1], &abilities[at]);
            if written.is_empty() {
                continue;
            }
            if let Some(slot) = self.find_text(kinds_of(&abilities[at - 1]), &written, false) {
                self.settle(at - 1, slot, ClauseState::Reads { ability: at - 1 });
                self.taken[at] = true;
                self.slot_of[at] = Some(slot);
                self.places[at] = self.places[at - 1];
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
        self.ep23_three_costs();
        // An additional cost and the spell's reduction for what it pays are one clause wherever the script lists them.
        for first in 0..abilities.len() {
            if self.taken[first] || !matches!(abilities[first], Ability::AdditionalCost { .. }) {
                continue;
            }
            for second in 0..abilities.len() {
                if self.taken[second]
                    || second == first
                    || !matches!(abilities[second], Ability::CostModifier { .. })
                {
                    continue;
                }
                let written = static_pair_texts(&abilities[first], &abilities[second]);
                if written.is_empty() {
                    continue;
                }
                if let Some(slot) = self.find_text(&[ClauseKind::Static], &written, false) {
                    self.settle(first, slot, ClauseState::Reads { ability: first });
                    self.taken[second] = true;
                    self.slot_of[second] = Some(slot);
                    self.places[second] = self.places[first];
                    break;
                }
            }
        }
    }

    /// An additional cost, the spell's reduction for what it pays and a second reduction of the spell are one clause
    /// wherever the script lists them (Dargo, the Shipwrecker).
    fn ep23_three_costs(&mut self) {
        let abilities = self.abilities;
        let free = |taken: &[bool], at: usize, cost: bool| {
            !taken[at]
                && if cost {
                    matches!(abilities[at], Ability::AdditionalCost { .. })
                } else {
                    matches!(abilities[at], Ability::CostModifier { .. })
                }
        };
        for first in 0..abilities.len() {
            if !free(&self.taken, first, true) {
                continue;
            }
            'pair: for second in 0..abilities.len() {
                if second == first || !free(&self.taken, second, false) {
                    continue;
                }
                for third in 0..abilities.len() {
                    if third == first || third == second || !free(&self.taken, third, false) {
                        continue;
                    }
                    let written =
                        ep23_three_texts(&abilities[first], &abilities[second], &abilities[third]);
                    if written.is_empty() {
                        continue;
                    }
                    if let Some(slot) = self.find_text(&[ClauseKind::Static], &written, false) {
                        self.settle(first, slot, ClauseState::Reads { ability: first });
                        for later in [second, third] {
                            self.taken[later] = true;
                            self.slot_of[later] = Some(slot);
                            self.places[later] = self.places[first];
                        }
                        break 'pair;
                    }
                }
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
                for slot in self.c11f_spell_slots(&written, rolls_dice(ability)) {
                    self.settle(at, slot, ClauseState::Reads { ability: at });
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
                if wanted.is_empty() {
                    state = Some(ClauseState::Unwritten {
                        ability: *ability,
                        construct: "die roll row (the effects of the row have no writer)"
                            .to_owned(),
                    });
                } else if !wanted
                    .iter()
                    .any(|written| same_written_text(written, text))
                {
                    let written = closest(wanted.to_vec(), text).unwrap_or_default();
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
            .filter(|&slot| {
                !(matches!(ability, Ability::Spell { .. })
                    && rolls_dice(ability)
                    && is_die_row(&self.slots[slot].clause.text))
            })
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

    /// The clauses a spell ability reads as one text: its spell and modal lines, else those with the lines the compiler
    /// takes for statics ("Metalcraft — If you control three or more artifacts, exile that creature."), which a whole
    /// wording of the spell then has to name.
    fn c11f_spell_slots(&self, written: &[String], rolls: bool) -> Vec<usize> {
        let spell = [ClauseKind::Spell, ClauseKind::Modal];
        let wider = [ClauseKind::Spell, ClauseKind::Modal, ClauseKind::Static];
        for kinds in [&spell[..], &wider[..]] {
            let mut open = self.open_slots(kinds);
            if rolls {
                // The rows of a die roll's table are read after the roll (`tables`).
                open.retain(|&slot| !is_die_row(&self.slots[slot].clause.text));
            }
            let joined = self.joined(&open);
            if written.iter().any(|variant| self.same(variant, &joined)) {
                return open;
            }
        }
        Vec::new()
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
    match closest(written, wanted) {
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

/// The sentence of a haunt creature's two abilities: `enters` is "When ~ enters, X" and `haunted` the same trigger from
/// exile on "the creature it haunts dies"; none for any other pair.
pub(crate) fn haunt_pair_texts(enters: &Ability, haunted: &Ability) -> Vec<String> {
    let (
        Ability::Triggered {
            when: first,
            intervening_if: None,
            limit: None,
            from: crate::refs::ZoneKind::Battlefield,
            targets,
            effects,
        },
        Ability::Triggered {
            when: second,
            intervening_if: None,
            limit: None,
            from: crate::refs::ZoneKind::Exile,
            targets: other_targets,
            effects: other_effects,
        },
    ) = (enters, haunted)
    else {
        return Vec::new();
    };
    let this = crate::refs::Selector::this();
    let haunts = crate::refs::Selector(vec![crate::refs::Pred::HauntedBy(crate::refs::Ref::This)]);
    let entering = matches!(first, TriggerPattern::ZoneChange { what, from: None, to: Some(crate::refs::ZoneKind::Battlefield) } if *what == this);
    let dying = matches!(second, TriggerPattern::ZoneChange { what, from: Some(crate::refs::ZoneKind::Battlefield), to: Some(crate::refs::ZoneKind::Graveyard) } if *what == haunts);
    if !entering || !dying || targets != other_targets || effects != other_effects {
        return Vec::new();
    }
    written_variants(enters)
        .into_iter()
        .filter_map(|text| {
            text.strip_prefix("When ~ enters, ")
                .map(|rest| format!("When ~ enters or the creature it haunts dies, {rest}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a row of the table of a die roll is told from other clauses by its range of totals and its bar, whatever the range looks like, and only an ability that rolls a die owns the rows after its clause; a spell ability reads a line the compiler takes for a static only when its wording names every line; what a source is attached to is named by the face (equipped for an Equipment, enchanted for an Aura), and the other word reads as another rule.
    use rstest::rstest;

    use super::*;

    fn c11f_states(type_line: &str, oracle_text: &str, proposal: &str) -> Vec<ClauseState> {
        let printed = PrintedCard {
            name: "Test Spell".into(),
            faces: vec![crate::support::PrintedFace {
                name: "Test Spell".into(),
                type_line: type_line.into(),
                oracle_text: oracle_text.into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let value = super::super::complete::read(proposal, "cards/test_spell.json")
            .expect("the proposal is a JSON object");
        let card = super::super::complete::parse(value, &printed, "cards/test_spell.json")
            .expect("the proposal parses");
        let clauses = crate::compile::compile(&printed).clauses;
        pair_card(&card, &clauses, &printed)
            .clauses
            .into_iter()
            .map(|clause| clause.state)
            .collect()
    }

    /// Tap, then an exile under Metalcraft (Dispatch): one spell ability of two lines, the second one a static's.
    const C11F_DISPATCH: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":3}},"then":[{"Exile":{"what":{"Target":0}}}]}}]}}]}]}"#;

    #[rstest]
    #[case::both_lines(
        "Tap target creature.\nMetalcraft \u{2014} If you control three or more artifacts, exile that creature.",
        true
    )]
    #[case::another_count(
        "Tap target creature.\nMetalcraft \u{2014} If you control four or more artifacts, exile that creature.",
        false
    )]
    #[case::the_first_line_alone("Tap target creature.", false)]
    fn c11f_a_spell_reads_a_static_line_only_whole(#[case] text: &str, #[case] reads: bool) {
        let states = c11f_states("Instant", text, C11F_DISPATCH);
        let all_read = states
            .iter()
            .all(|state| matches!(state, ClauseState::Reads { ability: 0 }));
        assert_eq!(all_read, reads, "{states:?}");
    }

    /// An activated ability that pumps what the source is attached to (Demonspine Whip).
    const W16_PUMP: &str = r#"{"faces":[{"abilities":[{"Activated":{"cost":"{X}","effects":[{"Continuous":{"affected":"Attached","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":"X","toughness":0}}]}}]}}]}]}"#;

    #[rstest]
    #[case::equipment_equipped("Artifact \u{2014} Equipment", "Equipped", true)]
    #[case::equipment_enchanted("Artifact \u{2014} Equipment", "Enchanted", false)]
    #[case::aura_enchanted("Enchantment \u{2014} Aura", "Enchanted", true)]
    #[case::aura_equipped("Enchantment \u{2014} Aura", "Equipped", false)]
    #[case::other_card_equipped("Artifact", "Equipped", false)]
    #[case::other_card_enchanted("Artifact", "Enchanted", true)]
    fn w16_the_attached_object_is_named_by_the_face(
        #[case] type_line: &str,
        #[case] word: &str,
        #[case] reads: bool,
    ) {
        let text = format!("{{X}}: {word} creature gets +X/+0 until end of turn.");
        let states = c11f_states(type_line, &text, W16_PUMP);
        let read = states
            .iter()
            .all(|state| matches!(state, ClauseState::Reads { ability: 0 }));
        assert_eq!(read, reads, "{states:?}");
    }

    const FIX27_GRANT: &str = r#"{"faces":[{"abilities":[{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Lifelink"}]}}}],"condition":{"Matches":{"subject":"Attached","selector":[{"Subtype":"Equipment"}]}}}}]}]}"#;

    #[rstest]
    #[case::aura_grants_to_an_equipment("enchanted permanent", "it", "Equipped", "lifelink", true)]
    #[case::aura_grants_another_keyword(
        "enchanted permanent",
        "it",
        "Equipped",
        "deathtouch",
        false
    )]
    #[case::aura_names_its_own_object_equipped(
        "equipped creature",
        "it",
        "Equipped",
        "lifelink",
        false
    )]
    #[case::aura_gives_to_equipped_creature(
        "enchanted permanent",
        "equipped creature",
        "Equipped",
        "lifelink",
        false
    )]
    fn fix27_a_quoted_ability_keeps_its_own_attachment(
        #[case] subject: &str,
        #[case] holder: &str,
        #[case] word: &str,
        #[case] keyword: &str,
        #[case] reads: bool,
    ) {
        let type_line = "Enchantment \u{2014} Aura";
        let text = format!(
            "As long as {subject} is an Equipment, {holder} has \"{word} creature has {keyword}.\""
        );
        let states = c11f_states(type_line, &text, FIX27_GRANT);
        let read = states
            .iter()
            .all(|state| matches!(state, ClauseState::Reads { ability: 0 }));
        assert_eq!(read, reads, "{states:?}");
    }

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

#[cfg(test)]
mod c11g_stripped_tests {
    //! Goal: an ability word or the numerals of a chapter leave the clause, while the name of a mode on a later line stays.
    use rstest::rstest;

    use super::stripped;

    #[rstest]
    #[case::ability_word(
        "Landfall — Whenever a land you control enters, draw a card.",
        "Whenever a land you control enters, draw a card."
    )]
    #[case::chapter("I, II — Draw a card.", "Draw a card.")]
    #[case::mode_name(
        "Choose one —\n• Disarm — Destroy target artifact.\n• Detonate — Draw a card.",
        "Choose one —\n• Disarm — Destroy target artifact.\n• Detonate — Draw a card."
    )]
    #[case::self_named("~ — Draw a card.", "~ — Draw a card.")]
    #[case::wf27_six_words(
        "A Thousand Souls Die Every Day — {T}, Sacrifice a creature: Add three mana of any one color.",
        "{T}, Sacrifice a creature: Add three mana of any one color."
    )]
    #[case::wf27_a_sentence(
        "When this creature enters, return target card to its owner's hand — {T}: Draw a card.",
        "When this creature enters, return target card to its owner's hand — {T}: Draw a card."
    )]
    #[case::no_dash("Draw a card.", "Draw a card.")]
    fn c11g_only_a_leading_word_is_stripped(#[case] text: &str, #[case] kept: &str) {
        assert_eq!(stripped(text), kept);
    }

    #[rstest]
    #[case::adamant(
        "Draw three cards. Adamant \u{2014} If at least three blue mana was spent to cast this spell, scry 3.",
        Some(
            "Draw three cards. If at least three blue mana was spent to cast this spell, scry 3."
        )
    )]
    #[case::two_words(
        "Draw a card. Spell mastery \u{2014} If X, scry 1.",
        Some("Draw a card. If X, scry 1.")
    )]
    #[case::next_clause(
        "Draw a card.\nDelirium \u{2014} If X, scry 1.",
        Some("Draw a card.\nIf X, scry 1.")
    )]
    #[case::leading_only("Raid \u{2014} When ~ enters, draw a card.", None)]
    // A mode's name on a later line, or a dash inside a sentence, stays.
    #[case::mode("Choose one \u{2014}\n\u{2022} Disarm \u{2014} Draw a card.", None)]
    #[case::lowercase("Draw a card. then \u{2014} scry 1.", None)]
    #[case::quoted("Draw a card. \"Hi \u{2014} there\"", None)]
    fn r7c_ability_words_of_later_sentences_are_flavor(
        #[case] text: &str,
        #[case] unworded: Option<&str>,
    ) {
        assert_eq!(
            crate::proposal::pairing::r7c_unworded(text).as_deref(),
            unworded
        );
    }
}

/// Whether the ability is a spell that chooses one of modes that each add a cost: a Tiered spell.
fn tiered_spell(ability: &Ability) -> bool {
    let Ability::Spell { effects, .. } = ability else {
        return false;
    };
    matches!(
        effects.as_slice(),
        [Effect::Modal { choose, mode_costs, modes, .. }]
            if choose.is_one() && !mode_costs.is_empty() && mode_costs.len() == modes.len()
    )
}

/// The sentences both of two different abilities of one kind write; none for two equal abilities, which a card never
/// prints as one sentence.
fn shared_texts(first: &Ability, second: &Ability) -> Vec<String> {
    if first == second || kinds_of(first) != kinds_of(second) {
        return Vec::new();
    }
    let theirs = written_variants(second);
    written_variants(first)
        .into_iter()
        .filter(|text| theirs.contains(text))
        .collect()
}

#[cfg(test)]
mod tiered_tests {
    //! Goal: only a spell that chooses exactly one of modes that each carry a cost is Tiered.
    use rstest::rstest;

    use super::tiered_spell;
    use crate::ability::Ability;

    const MODES: &str =
        r#""modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":2}}]}]"#;

    #[rstest]
    #[case::one_with_costs(r#""mode_costs":["{0}","{1}"],"#, true)]
    #[case::spree(r#""choose":{"min":1,"max":2},"mode_costs":["{1}","{1}"],"#, false)]
    #[case::no_costs("", false)]
    fn a_tiered_spell(#[case] fields: &str, #[case] tiered: bool) {
        let spell: Ability = serde_json::from_str(&format!(
            r#"{{"Spell":{{"effects":[{{"Modal":{{{fields}{MODES}}}}}]}}}}"#
        ))
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(tiered_spell(&spell), tiered);
    }
}

#[cfg(test)]
mod shared_tests {
    //! Goal: two different abilities of one kind read as one sentence when both write it, and never two equal abilities or abilities without a common sentence.
    use rstest::rstest;

    use super::shared_texts;
    use crate::ability::Ability;

    const VAMPIRE: &str = r#""when":{"SpellCast":{"what":[{"Subtype":"Vampire"},"Other"],"by":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Vampire","colors":["Black"],"types":["Creature"],"subtypes":["Vampire"],"power":1,"toughness":1}}}}]"#;
    const TOP: &str = r#"{"Rule":{"rule":{"LookAtTopAnyTime":"You"}}}"#;
    const FACE_DOWN: &str = r#"{"Rule":{"rule":{"LookAtFaceDown":{"what":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}}}"#;
    const EMINENCE: &str = "Eminence — Whenever you cast another Vampire spell, if ~ is in the command zone or on the battlefield, create a 1/1 black Vampire creature token.";
    const LENS: &str = "You may look at the top card of your library and at face-down creatures you don't control any time.";

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap_or_else(|error| panic!("{error}"))
    }

    fn vampire(from: &str) -> String {
        format!(r#"{{"Triggered":{{{VAMPIRE}{from}}}}}"#)
    }

    #[rstest]
    #[case::eminence(vampire(""), vampire(r#","from":"Command""#), Some(EMINENCE))]
    #[case::two_looks(TOP.to_owned(), FACE_DOWN.to_owned(), Some(LENS))]
    #[case::the_same_ability_twice(vampire(""), vampire(""), None)]
    #[case::nothing_in_common(TOP.to_owned(), vampire(""), None)]
    fn two_abilities_share_a_sentence(
        #[case] first: String,
        #[case] second: String,
        #[case] shared: Option<&str>,
    ) {
        let found = shared_texts(&ability(&first), &ability(&second));
        match shared {
            Some(text) => assert!(found.iter().any(|found| found == text), "{found:?}"),
            None => assert!(found.is_empty(), "{found:?}"),
        }
    }
}

#[cfg(test)]
mod e14f_tests {
    //! Goal: a haunt creature's enters trigger and its trigger from exile on the haunted creature's death read as the one sentence "When ~ enters or the creature it haunts dies, ..." only when both do the same, and the trigger from exile alone reads as "When the creature ~ haunts dies".
    use rstest::rstest;

    use super::haunt_pair_texts;
    use crate::ability::Ability;
    use crate::compile::{same_text, written_variants};

    const DESTROY: &str = r#""targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]"#;
    const ENTERS: &str = r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#;
    const HAUNTED: &str =
        r#"{"ZoneChange":{"what":[{"HauntedBy":"This"}],"from":"Battlefield","to":"Graveyard"}}"#;

    fn triggered(when: &str, from: &str, body: &str) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Triggered":{{"when":{when},{from}{body}}}}}"#
        ))
        .unwrap_or_else(|error| panic!("{error}"))
    }

    #[rstest]
    #[case::the_same(DESTROY, r#""from":"Exile","#, true)]
    #[case::another_effect(r#""effects":[{"Draw":{}}]"#, r#""from":"Exile","#, false)]
    #[case::not_from_exile(DESTROY, "", false)]
    fn the_pair_is_one_sentence(#[case] second: &str, #[case] from: &str, #[case] same: bool) {
        let enters = triggered(ENTERS, "", DESTROY);
        let haunted = triggered(HAUNTED, from, second);
        let found = haunt_pair_texts(&enters, &haunted);
        let clause = "When ~ enters or the creature it haunts dies, destroy target enchantment.";
        assert_eq!(
            found.iter().any(|text| same_text(text, clause)),
            same,
            "{found:?}"
        );
    }

    #[test]
    fn the_trigger_from_exile_alone_names_the_haunted_creature() {
        let haunted = triggered(HAUNTED, r#""from":"Exile","#, DESTROY);
        let found = written_variants(&haunted);
        let clause = "When the creature ~ haunts dies, destroy target enchantment.";
        assert!(
            found.iter().any(|text| same_text(text, clause)),
            "{found:?}"
        );
    }
}

#[cfg(test)]
mod w19r_tests {
    //! Goal: the wording shown for a differing clause is the closest one that names the card as the clause does, so a card is "it" unless the clause says "he" or "she".
    use super::closest;

    fn owned(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| (*text).to_owned()).collect()
    }

    #[test]
    fn a_gendered_wording_is_shown_only_for_a_gendered_clause() {
        let variants = owned(&[
            "When ~ enters, she deals 1 damage.",
            "When ~ enters, it deals 1 damage.",
        ]);
        assert_eq!(
            closest(variants.clone(), "When ~ enters, it deals 2 damage."),
            Some("When ~ enters, it deals 1 damage.".to_owned())
        );
        assert_eq!(
            closest(variants, "When ~ enters, she deals 2 damage."),
            Some("When ~ enters, she deals 1 damage.".to_owned())
        );
    }

    #[test]
    fn nothing_is_shown_without_a_wording() {
        assert_eq!(closest(Vec::new(), "Draw a card."), None);
    }
}
