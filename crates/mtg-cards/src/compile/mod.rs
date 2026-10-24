//! The oracle-text compiler: an imported card's text becomes rules data, and what the compiler
//! cannot read stays a `Manual` clause with its span (`docs/cards-dsl.md`, Compiler and Manual fallback).
//!
//! Six pure stages: `normalize` strips reminder text and the card's own name, `clauses` splits the
//! text, `classify` names each clause's shape, `keywords` reads keyword lines, `read` holds the
//! templates that read spell effects and mana abilities (with the renderer that checks them) and
//! `manual` leaves every other clause to the table.

mod classify;
mod clauses;
mod keywords;
mod linked;
mod manual;
mod normalize;
mod read;

use std::collections::BTreeSet;

use crate::ability::Ability;
use crate::card::{Card, Face, ImplStatus, Span};
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::support::{PrintedCard, PrintedFace, has_rules_text, oracle_fingerprint};

pub use classify::ClauseKind;
pub use keywords::read as read_keyword;
pub use keywords::read_all as read_keywords;
pub use read::{
    render_ability, render_mode, render_variants, same_text, static_pair_texts, written_variants,
};

/// One clause of a face's oracle text, read or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    /// The face, as an index into the card's faces.
    pub face: usize,
    /// Where the clause lies in the face's imported oracle text.
    pub span: Span,
    /// The clause as the compiler saw it: reminder text removed and the card's name as `~`.
    pub text: String,
    /// Its shape.
    pub kind: ClauseKind,
    /// Whether the compiler turned it into rules; otherwise it is a `Manual` ability.
    pub read: bool,
}

/// What the compiler made of one card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    /// The rules, or `None` when no face has rules text once reminder text is removed, or the data
    /// cannot hold the card (over four faces, or two faces with one name).
    pub card: Option<Card>,
    /// Every clause of every face in text order, read or not, for the coverage report.
    pub clauses: Vec<Clause>,
}

/// Compiles an imported card: every clause is read or left to the table as a `Manual` ability.
///
/// The status is `Full` when every clause is read, `Partial` when some are, and `Manual` (no
/// abilities) when none is. Every face of a `Full` or `Partial` card carries the fingerprint of its text.
pub fn compile(printed: &PrintedCard) -> Compiled {
    compile_among(printed, None)
}

/// Compiles an imported card among the cards `known` (their lowercase names) when the library is at hand: a "Partner
/// with" that names no known card has no partner to find, so its line is left to the table. Without `known` every
/// name stands.
pub fn compile_among(printed: &PrintedCard, known: Option<&BTreeSet<String>>) -> Compiled {
    let mut listed = Vec::new();
    let mut faces = Vec::with_capacity(printed.faces.len());
    for (index, shown) in printed.faces.iter().enumerate() {
        faces.push(Face {
            name: shown.name.clone(),
            oracle_fingerprint: oracle_fingerprint(&shown.oracle_text),
            abilities: read_face(index, shown, known, &mut listed),
        });
    }
    let card = build(printed, faces, &mut listed);
    Compiled {
        card,
        clauses: listed,
    }
}

/// A clause of a face, before its ability is placed.
struct Pending {
    span: Span,
    text: String,
    kind: ClauseKind,
    /// The ability it is read into; none for a clause left to the table and for a spell line
    /// that belongs to the spell ability of an earlier line.
    ability: Option<Ability>,
    read: bool,
    /// The abilities after the first that the clause reads into (a level block reads into several).
    more: Vec<Ability>,
    /// Its reminder text explains a counter, so the reading would lose what the counter does.
    reminds_counter: bool,
}

/// The abilities of one face; its clauses are appended to `listed`.
///
/// The spell clauses of an instant or sorcery are one `Spell` ability (the engine resolves the
/// first one of a face): read together when every one of them reads, left to the table otherwise.
fn read_face(
    index: usize,
    shown: &PrintedFace,
    known: Option<&BTreeSet<String>>,
    listed: &mut Vec<Clause>,
) -> Vec<Ability> {
    let partners = Partners::of(shown, known);
    let text = normalize::normalize(&shown.oracle_text, &own_names(shown));
    let (spell, aura) = (is_spell(shown), is_aura(shown));
    let x_free = shown.mana_cost.contains("{X}") || shown.oracle_text.lines().any(counts_x);
    let own = own_face(shown);
    let mut pending = Vec::new();
    for raw in clauses::split(&text) {
        let clause_text = text.slice(raw.range.clone());
        let Some(span) = text.span(raw.range, raw.ends_line) else {
            continue;
        };
        let mut kind = classify::classify(&clause_text, spell);
        if is_unread_keyword_line(&clause_text, kind, x_free, &own) {
            kind = ClauseKind::Keyword;
        }
        let reminds_counter = kind != ClauseKind::Keyword
            && span
                .slice(&shown.oracle_text)
                .is_some_and(normalize::reminder_mentions_counter);
        let (ability, more) = read_clause(
            &clause_text,
            kind,
            (aura, reminds_counter),
            x_free,
            (&own, partners),
        );
        pending.push(Pending {
            span,
            text: clause_text,
            kind,
            read: ability.is_some(),
            ability,
            more,
            reminds_counter,
        });
    }
    let lines: Vec<&str> = pending
        .iter()
        .filter(|clause| clause.kind == ClauseKind::Spell)
        .map(|clause| clause.text.as_str())
        .collect();
    let spell_text = lines.join("\n");
    let counter_explained = pending
        .iter()
        .any(|clause| clause.kind == ClauseKind::Spell && clause.reminds_counter);
    let spell_ability = if spell_text.is_empty() || counter_explained {
        None
    } else {
        read::clause_with(&spell_text, ClauseKind::Spell, x_free, &own)
    };
    // A cost that counts targets needs the targets of the spell ability beside it: without them it is left to the table.
    let has_targets = spell_ability
        .as_ref()
        .is_some_and(|ability| !ability.targets().is_empty());
    if !has_targets {
        unread_target_counts(&mut pending);
    }
    if let Some(ability) = spell_ability {
        let mut ability = Some(ability);
        for clause in pending
            .iter_mut()
            .filter(|clause| clause.kind == ClauseKind::Spell)
        {
            clause.read = true;
            clause.ability = ability.take();
        }
    }
    if spell && spell_text.is_empty() {
        // The modes of an instant or sorcery are its whole spell ability when no other line is.
        let mut modals = pending
            .iter_mut()
            .filter(|clause| clause.kind == ClauseKind::Modal);
        if let (Some(only), None) = (modals.next(), modals.next())
            && !only.reminds_counter
        {
            only.ability = read::clause_with(&only.text, ClauseKind::Modal, x_free, &own);
            only.read = only.ability.is_some();
        }
    }
    give_backup(&mut pending);
    linked::exile_pair(&mut pending);
    let mut abilities = Vec::new();
    for clause in pending {
        match clause.ability {
            Some(ability) => {
                abilities.push(ability);
                abilities.extend(clause.more);
            }
            None if !clause.read => abilities.push(manual::manual(clause.kind, clause.span)),
            None => {}
        }
        listed.push(Clause {
            face: index,
            span: clause.span,
            text: clause.text,
            kind: clause.kind,
            read: clause.read,
        });
    }
    abilities
}

/// The names a "Partner with" line may use: the card's own and those of the cards the library holds.
#[derive(Clone, Copy)]
struct Partners<'a> {
    own: &'a str,
    known: Option<&'a BTreeSet<String>>,
}

impl<'a> Partners<'a> {
    /// The names for a face among the cards `known`, when the library is at hand.
    fn of(face: &'a PrintedFace, known: Option<&'a BTreeSet<String>>) -> Self {
        Partners {
            own: &face.name,
            known,
        }
    }

    /// The keyword as the card names its partner: "Partner with itself" names the card (a commander deck holds two of
    /// it, CR 702.124), and a name no card bears ("Partner with Knight") is no partner, so its line is left to the table.
    fn named(self, keyword: Keyword) -> Option<Keyword> {
        let Keyword::PartnerWith(name) = keyword else {
            return Some(keyword);
        };
        let name = if name.eq_ignore_ascii_case("itself") {
            self.own.to_owned()
        } else {
            name
        };
        self.known
            .is_none_or(|known| known.contains(&name.to_lowercase()))
            .then_some(Keyword::PartnerWith(name))
    }
}

/// The ability a clause reads into, and the abilities after it that the same clause reads into (a level block reads into
/// one ability for each of its lines). `aura` says whether the face is an Aura and `reminds_counter` whether the
/// reminder text of the clause explains a counter, which the reading would lose.
fn read_clause(
    text: &str,
    kind: ClauseKind,
    (aura, reminds_counter): (bool, bool),
    x_free: bool,
    (own, partners): (&read::Own, Partners<'_>),
) -> (Option<Ability>, Vec<Ability>) {
    match kind {
        ClauseKind::Keyword => {
            // `check_printed` accepts Enchant on an Aura only.
            let mut keywords = keywords::read_all(text)
                .unwrap_or_default()
                .into_iter()
                .filter(|keyword| aura || !matches!(keyword, Keyword::Enchant(_)))
                .filter_map(|keyword| partners.named(keyword))
                .map(Ability::Keyword);
            (keywords.next(), keywords.collect())
        }
        ClauseKind::Static
            if text.starts_with("LEVEL ")
                || clauses::is_station_block(text)
                || clauses::is_class_block(text) =>
        {
            let block = if text.starts_with("LEVEL ") {
                read::level_block(text, x_free, own)
            } else if clauses::is_class_block(text) {
                read::class_block(text, x_free, own)
            } else {
                read::station_block(text, x_free, own)
            };
            let mut block = block.unwrap_or_default().into_iter();
            (block.next(), block.collect())
        }
        ClauseKind::Triggered if !reminds_counter && is_chapter_line(text) => {
            let mut block = read::chapter_block(text, x_free, own)
                .unwrap_or_default()
                .into_iter();
            (block.next(), block.collect())
        }
        ClauseKind::Triggered | ClauseKind::Activated | ClauseKind::Static if !reminds_counter => {
            match read::clause_with(text, kind, x_free, own) {
                // Two statics in one sentence after one condition read into two abilities.
                None if kind == ClauseKind::Static => {
                    let mut pair = read::static_pair(text, x_free, own)
                        .unwrap_or_default()
                        .into_iter();
                    (pair.next(), pair.collect())
                }
                single => (single, Vec::new()),
            }
        }
        _ => (None, Vec::new()),
    }
}

/// Whether the clause is a Saga chapter line ("I — ...", "II, III — ...").
fn is_chapter_line(text: &str) -> bool {
    text.lines()
        .next()
        .is_some_and(|head| classify::chapter(head).is_some())
}

/// Leaves the cost modifiers that count targets ("for each target beyond the first") to the table.
fn unread_target_counts(pending: &mut [Pending]) {
    for clause in pending {
        if matches!(&clause.ability, Some(Ability::CostModifier { per, .. }) if read::counts_targets(per))
        {
            clause.ability = None;
            clause.read = false;
        }
    }
}

/// Backup lists the abilities that follow it on the face (the other backups aside), which its target gains: a backup
/// followed by a clause left to the table, or by an ability that cannot be granted, is left to the table too.
fn give_backup(pending: &mut [Pending]) {
    for at in 0..pending.len() {
        if !matches!(
            pending[at].ability,
            Some(Ability::Keyword(Keyword::Backup { .. }))
        ) {
            continue;
        }
        let mut listed = Vec::new();
        let mut all_read = true;
        for clause in &pending[at + 1..] {
            match &clause.ability {
                Some(Ability::Keyword(Keyword::Backup { .. })) => {}
                Some(ability) if can_be_granted(ability) => listed.push(ability.clone()),
                _ => all_read = false,
            }
        }
        if all_read {
            if let Some(Ability::Keyword(Keyword::Backup { abilities, .. })) =
                &mut pending[at].ability
            {
                *abilities = listed;
            }
        } else {
            pending[at].ability = None;
            pending[at].read = false;
        }
    }
}

/// Whether an ability of a face can be given to another creature: a keyword, or an activated, triggered or static ability
/// that does not define characteristics or give abilities in turn.
fn can_be_granted(ability: &Ability) -> bool {
    match ability {
        Ability::Keyword(_) | Ability::Activated { .. } | Ability::Triggered { .. } => true,
        Ability::Static { mods, defining, .. } => {
            !defining
                && !mods
                    .iter()
                    .any(|modification| matches!(modification, Modification::GrantAbility(_)))
        }
        _ => false,
    }
}

/// Whether a spell or static clause is a keyword line the keyword reader does not know
/// (`Cleave {1}{B}`): one line without a closing period that no template reads. It is kept as a
/// keyword clause so that it does not stand in the way of the effect lines around it.
fn is_unread_keyword_line(text: &str, kind: ClauseKind, x_free: bool, own: &read::Own) -> bool {
    matches!(kind, ClauseKind::Spell | ClauseKind::Static)
        && !clauses::is_station_block(text)
        && !text.contains('\n')
        && !text.trim_end().ends_with(['.', '"', '\u{201d}'])
        && read::clause_with(text, kind, x_free, own).is_none()
}

/// The face name, and for a legendary face the part of the name before the comma.
fn own_names(face: &PrintedFace) -> Vec<String> {
    let mut names = vec![face.name.clone()];
    let legendary = face
        .type_line
        .split_whitespace()
        .any(|word| word == "Legendary");
    if legendary && let Some((short, _)) = face.name.split_once(',') {
        names.push(short.trim().to_owned());
    } else if legendary && let Some(first) = first_name(face) {
        names.push(first);
    }
    names
}

/// The first word of a legendary name without a comma when the text uses it alone ("Legolas" for "Legolas Greenleaf").
fn first_name(face: &PrintedFace) -> Option<String> {
    let first = face.name.split_once(' ')?.0;
    let word = first.chars().all(char::is_alphabetic)
        && first.chars().next().is_some_and(char::is_uppercase)
        && first.chars().count() > 2
        && !["The", "Der", "Die", "Los", "Las", "Les"].contains(&first);
    let text = face.oracle_text.replace(&face.name, "");
    let alone = text.match_indices(first).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = &text[at + first.len()..];
        // Not the start of another name ("Partner with Test Trynn").
        let next_word = after.strip_prefix(' ').and_then(|rest| rest.chars().next());
        !before.is_some_and(char::is_alphanumeric)
            && !after.starts_with(|c: char| c.is_alphabetic())
            && !next_word.is_some_and(char::is_uppercase)
    });
    (word && alone).then(|| first.to_owned())
}

/// Whether a line of text is a required additional cost that takes objects out of their zones.
fn requires_a_pick(line: &str) -> bool {
    let line = line.to_lowercase();
    line.strip_prefix("as an additional cost to cast this spell, ")
        .is_some_and(|cost| {
            ["sacrifice", "discard", "exile"]
                .iter()
                .any(|verb| cost.starts_with(verb))
        })
}

/// Whether a line of text is a required additional cost that counts the X the caster announces ("As an additional cost
/// to cast this spell, pay X life."), which the effects of the spell read. The X of "with mana value X" is no amount the
/// caster announces: it names the mana value of the permanent the cost takes, which the table plays.
fn counts_x(line: &str) -> bool {
    let line = line.to_lowercase();
    line.strip_prefix("as an additional cost to cast this spell, ")
        .is_some_and(|cost| {
            let announced = ["mana value", "power", "toughness"]
                .iter()
                .fold(cost.to_owned(), |cost, stat| {
                    cost.replace(&format!("{stat} x"), stat)
                });
            !cost.starts_with("you may ")
                && announced
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|word| word == "x")
        })
}

/// What the readers take from the face besides its text: the subtypes on its type line, which count as subtypes
/// whether or not the rules' list has them, and the power and toughness when they are printed as numbers.
fn own_face(face: &PrintedFace) -> read::Own {
    let number = |printed: &Option<String>| printed.as_deref().and_then(|text| text.parse().ok());
    read::Own {
        subtypes: face
            .type_line
            .split_once('\u{2014}')
            .map(|(_, subtypes)| subtypes.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default(),
        power: number(&face.power),
        toughness: number(&face.toughness),
        creature_at: number(&face.power).and_then(|_: i32| {
            face.oracle_text
                .lines()
                .filter_map(|line| clauses::station_header(line).map(|(threshold, _)| threshold))
                .max()
        }),
        pays: face.oracle_text.lines().any(requires_a_pick),
    }
}

/// Whether the face is an Aura.
fn is_aura(face: &PrintedFace) -> bool {
    face.type_line
        .split_once('\u{2014}')
        .is_some_and(|(_, subtypes)| subtypes.split_whitespace().any(|word| word == "Aura"))
}

/// Whether the face is an instant or a sorcery.
fn is_spell(face: &PrintedFace) -> bool {
    let types = face.type_line.split('\u{2014}').next().unwrap_or_default();
    types
        .split_whitespace()
        .any(|word| matches!(word, "Instant" | "Sorcery"))
}

/// Whether there is something to write and the data can hold it: some face has rules text, there
/// are at most four faces, and no two faces share a name.
fn is_writable(printed: &PrintedCard) -> bool {
    let names: BTreeSet<&str> = printed
        .faces
        .iter()
        .map(|face| face.name.as_str())
        .collect();
    printed
        .faces
        .iter()
        .any(|face| has_rules_text(&face.oracle_text))
        && printed.faces.len() <= 4
        && names.len() == printed.faces.len()
}

/// The card with its status, or `None` when it is not writable.
fn build(printed: &PrintedCard, mut faces: Vec<Face>, listed: &mut [Clause]) -> Option<Card> {
    if !is_writable(printed) {
        return None;
    }
    let read = listed.iter().filter(|clause| clause.read).count();
    let unread = listed.len() - read;
    // A `Full` instant or sorcery must have a `Spell` ability (`check_printed`), which only keyword lines cannot give.
    let spell_missing = printed.faces.iter().zip(&faces).any(|(shown, face)| {
        is_spell(shown)
            && !face
                .abilities
                .iter()
                .any(|ability| matches!(ability, Ability::Spell { .. }))
    });
    let impl_status = if read == 0 || (unread == 0 && spell_missing) {
        ImplStatus::Manual
    } else if unread == 0 {
        ImplStatus::Full
    } else {
        ImplStatus::Partial
    };
    if impl_status == ImplStatus::Manual {
        for face in &mut faces {
            face.abilities.clear();
            face.oracle_fingerprint.clear();
        }
        for clause in listed.iter_mut() {
            clause.read = false;
        }
    }
    Some(Card {
        oracle_id: printed.oracle_id.clone(),
        name: printed.name.clone(),
        impl_status,
        faces,
        notes: None,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a card compiles to the status its clauses earn, with fingerprints, names and spans that the card files require.
    use super::*;
    use crate::keyword::Keyword;
    use rstest::rstest;

    fn face(name: &str, type_line: &str, oracle_text: &str) -> PrintedFace {
        PrintedFace {
            name: name.into(),
            type_line: type_line.into(),
            oracle_text: oracle_text.into(),
            ..PrintedFace::default()
        }
    }

    fn card(faces: Vec<PrintedFace>) -> PrintedCard {
        let name = faces
            .iter()
            .map(|face| face.name.as_str())
            .collect::<Vec<_>>()
            .join(" // ");
        PrintedCard {
            layout: String::new(),
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name,
            keywords: vec![],
            faces,
        }
    }

    fn one(name: &str, type_line: &str, text: &str) -> Compiled {
        compile(&card(vec![face(name, type_line, text)]))
    }

    fn status(compiled: &Compiled) -> Option<ImplStatus> {
        compiled.card.as_ref().map(|card| card.impl_status)
    }

    #[rstest]
    #[case::only_keywords(
        "Creature \u{2014} Angel",
        "Flying\nVigilance (Attacking doesn't cause this creature to tap.)",
        Some(ImplStatus::Full)
    )]
    #[case::keyword_list(
        "Creature \u{2014} Beast",
        "Vigilance, deathtouch, haste",
        Some(ImplStatus::Full)
    )]
    #[case::keyword_and_trigger(
        "Creature \u{2014} Elf",
        "Haste\nWhen ~ enters, draw a card.",
        Some(ImplStatus::Full)
    )]
    #[case::keyword_and_unread_trigger(
        "Creature \u{2014} Elf",
        "Haste\nWhen ~ enters, flip a coin.",
        Some(ImplStatus::Partial)
    )]
    #[case::trigger_with_an_ability_word(
        "Creature \u{2014} Elf",
        "Landfall \u{2014} Whenever a land you control enters, draw a card.",
        Some(ImplStatus::Full)
    )]
    #[case::keyword_and_static(
        "Artifact \u{2014} Equipment",
        "Equipped creature gets +2/+0.\nEquip {1}",
        Some(ImplStatus::Full)
    )]
    #[case::keyword_and_an_unread_static(
        "Artifact \u{2014} Equipment",
        "Equipped creature can't be blocked by more than two creatures.\nEquip {1}",
        Some(ImplStatus::Partial)
    )]
    #[case::nothing_read("Instant", "Flip a coin.", Some(ImplStatus::Manual))]
    #[case::spell_read_whole("Instant", "~ deals 3 damage to any target.", Some(ImplStatus::Full))]
    #[case::mana_ability_read("Creature \u{2014} Elf", "{T}: Add {G}.", Some(ImplStatus::Full))]
    #[case::x_counted_by_a_required_cost(
        "Instant",
        "As an additional cost to cast this spell, pay X life.\nDraw X cards.",
        Some(ImplStatus::Full)
    )]
    #[case::x_counted_by_a_required_pick(
        "Sorcery",
        "As an additional cost to cast this spell, sacrifice X creatures.\nDraw X cards.",
        Some(ImplStatus::Full)
    )]
    #[case::x_nobody_announces("Instant", "You draw X cards.", Some(ImplStatus::Manual))]
    #[case::x_of_an_optional_cost_is_not_announced(
        "Instant",
        "As an additional cost to cast this spell, you may pay X life.\nDraw X cards.",
        Some(ImplStatus::Manual)
    )]
    #[case::spell_lines_read_as_one_ability(
        "Sorcery",
        "Draw a card.\nYou gain 2 life.",
        Some(ImplStatus::Full)
    )]
    #[case::one_unread_spell_line_leaves_every_line(
        "Sorcery",
        "Draw a card.\nFlip a coin.",
        Some(ImplStatus::Manual)
    )]
    #[case::unreadable_keyword_only("Creature \u{2014} Elf", "Banding", Some(ImplStatus::Manual))]
    #[case::reminder_only("Basic Land \u{2014} Forest", "({T}: Add {G}.)", None)]
    #[case::vanilla("Creature \u{2014} Bear", "", None)]
    #[case::whitespace_only("Creature \u{2014} Bear", "  \n ", None)]
    #[case::spell_with_only_a_keyword("Instant", "Flash", Some(ImplStatus::Manual))]
    #[case::spell_with_keyword_and_effect(
        "Instant",
        "Kicker {1}\nDraw a card.",
        Some(ImplStatus::Full)
    )]
    #[case::spell_with_keyword_and_unread_effect(
        "Instant",
        "Kicker {1}\nFlip a coin.",
        Some(ImplStatus::Partial)
    )]
    #[case::modes_are_the_whole_spell(
        "Instant",
        "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.",
        Some(ImplStatus::Full)
    )]
    #[case::modes_after_a_keyword(
        "Sorcery",
        "Kicker {2}\nChoose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.",
        Some(ImplStatus::Full)
    )]
    #[case::an_unread_mode(
        "Instant",
        "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} Flip a coin.",
        Some(ImplStatus::Manual)
    )]
    #[case::modes_beside_spell_lines(
        "Instant",
        "Draw a card.\nChoose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.",
        Some(ImplStatus::Partial)
    )]
    #[case::two_modal_lists(
        "Instant",
        "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.\nChoose one \u{2014}\n\u{2022} Scry 1.\n\u{2022} Draw a card.",
        Some(ImplStatus::Manual)
    )]
    #[case::modes_on_a_permanent(
        "Enchantment",
        "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} You gain 2 life.",
        Some(ImplStatus::Manual)
    )]
    #[case::a_cost_beside_the_spell_it_counts_the_targets_of(
        "Sorcery",
        "This spell costs {1} more to cast for each target beyond the first.\nDestroy any number of target creatures.",
        Some(ImplStatus::Full)
    )]
    #[case::a_cost_that_counts_targets_the_spell_does_not_read(
        "Sorcery",
        "This spell costs {1} more to cast for each target beyond the first.\nAny number of target creatures phase out.",
        Some(ImplStatus::Manual)
    )]
    #[case::a_cost_that_counts_nothing_stands_alone(
        "Sorcery",
        "This spell costs {1} less to cast.\nFlip a coin.",
        Some(ImplStatus::Partial)
    )]
    fn the_status_follows_the_clauses(
        #[case] type_line: &str,
        #[case] text: &str,
        #[case] expected: Option<ImplStatus>,
    ) {
        assert_eq!(status(&one("Test Card", type_line, text)), expected);
    }

    #[test]
    fn a_full_card_has_a_keyword_per_clause_and_the_fingerprint_of_its_text() {
        let text = "Flying\nVigilance (Attacking doesn't cause this creature to tap.)";
        let compiled = one("Serra Angel", "Creature \u{2014} Angel", text);
        let card = compiled.card.unwrap();
        assert_eq!(card.name, "Serra Angel");
        assert_eq!(card.faces[0].oracle_fingerprint, oracle_fingerprint(text));
        assert_eq!(
            card.faces[0].abilities,
            [
                Ability::Keyword(Keyword::Flying),
                Ability::Keyword(Keyword::Vigilance)
            ]
        );
        assert!(compiled.clauses.iter().all(|clause| clause.read));
        assert_eq!(compiled.clauses[1].text, "Vigilance");
        assert_eq!(
            compiled.clauses[1].span.slice(text),
            Some("Vigilance (Attacking doesn't cause this creature to tap.)")
        );
    }

    #[test]
    fn a_partial_card_keeps_a_manual_ability_for_each_unread_clause_in_order() {
        let text = "Flying\nWhenever ~ attacks, flip a coin.\n{T}: Flip a coin.\nVigilance";
        let compiled = one("Test Card", "Creature \u{2014} Bird", text);
        let card = compiled.card.unwrap();
        let abilities = &card.faces[0].abilities;
        assert_eq!(abilities.len(), 4);
        assert_eq!(abilities[0], Ability::Keyword(Keyword::Flying));
        assert_eq!(abilities[3], Ability::Keyword(Keyword::Vigilance));
        let Ability::Manual { reason, span } = &abilities[1] else {
            panic!("{:?}", abilities[1]);
        };
        assert_eq!(reason, "a triggered ability the compiler does not read yet");
        assert_eq!(span.slice(text), Some("Whenever ~ attacks, flip a coin."));
        assert_eq!(
            abilities[2],
            Ability::Manual {
                reason: "an activated ability the compiler does not read yet".into(),
                span: Span { start: 40, end: 57 }
            }
        );
        let kinds: Vec<_> = compiled.clauses.iter().map(|clause| clause.kind).collect();
        assert_eq!(
            kinds,
            [
                ClauseKind::Keyword,
                ClauseKind::Triggered,
                ClauseKind::Activated,
                ClauseKind::Keyword
            ]
        );
        let read: Vec<_> = compiled.clauses.iter().map(|clause| clause.read).collect();
        assert_eq!(read, [true, false, false, true]);
    }

    #[test]
    fn the_spell_lines_of_a_face_are_one_spell_ability_and_every_line_is_read() {
        let text = "Kicker {1}\nScry 1.\nDraw a card.\nYou gain 2 life.";
        let compiled = one("Test Card", "Instant", text);
        let card = compiled.card.unwrap();
        assert_eq!(card.impl_status, ImplStatus::Full);
        let abilities = &card.faces[0].abilities;
        assert_eq!(abilities.len(), 2, "{abilities:?}");
        assert!(matches!(abilities[0], Ability::Keyword(_)));
        let Ability::Spell { effects, .. } = &abilities[1] else {
            panic!("{:?}", abilities[1]);
        };
        assert_eq!(effects.len(), 3);
        assert_eq!(compiled.clauses.len(), 4);
        assert!(compiled.clauses.iter().all(|clause| clause.read));
    }

    #[rstest]
    #[case::with_a_cost("Cleave {1}{B}")]
    #[case::with_a_number("Squad 2")]
    #[case::with_a_word("Epic")]
    #[case::with_a_dash("Impending 4\u{2014}{2}{U}")]
    #[case::alone("Surge")]
    fn an_unknown_keyword_line_does_not_block_the_effect_lines_of_its_spell(#[case] line: &str) {
        for text in [
            format!("{line}\nDestroy target creature."),
            format!("Destroy target creature.\n{line}"),
        ] {
            let compiled = one("Test Card", "Sorcery", &text);
            let card = compiled.card.unwrap();
            assert_eq!(card.impl_status, ImplStatus::Partial, "{text}");
            let abilities = &card.faces[0].abilities;
            assert_eq!(abilities.len(), 2, "{text}");
            assert_eq!(
                abilities
                    .iter()
                    .filter(|ability| ability.is_manual())
                    .count(),
                1
            );
            assert!(
                abilities
                    .iter()
                    .any(|ability| matches!(ability, Ability::Spell { .. }))
            );
            let found: Vec<_> = compiled
                .clauses
                .iter()
                .map(|clause| (clause.kind, clause.read))
                .collect();
            let wanted = if text.starts_with(line) {
                [(ClauseKind::Keyword, false), (ClauseKind::Spell, true)]
            } else {
                [(ClauseKind::Spell, true), (ClauseKind::Keyword, false)]
            };
            assert_eq!(found, wanted, "{text}");
        }
    }

    #[rstest]
    #[case::a_sentence("Sorcery", "Flip a coin.", ClauseKind::Spell)]
    #[case::a_readable_line("Sorcery", "Draw a card", ClauseKind::Spell)]
    #[case::a_closing_quote("Sorcery", "Flip a coin named \"A.\"", ClauseKind::Spell)]
    #[case::a_static_sentence(
        "Creature \u{2014} Elf",
        "Elves you control get +1/+1.",
        ClauseKind::Static
    )]
    #[case::a_static_keyword_like_line(
        "Creature \u{2014} Elf",
        "Cleave {1}{B}",
        ClauseKind::Keyword
    )]
    #[case::a_level_block(
        "Creature \u{2014} Wizard",
        "LEVEL 2-3\n2/4\nFlying",
        ClauseKind::Static
    )]
    #[case::a_station_block_of_one_line(
        "Artifact \u{2014} Spacecraft",
        "8+ | Frobnicate",
        ClauseKind::Static
    )]
    fn only_a_line_without_a_period_that_nothing_reads_becomes_a_keyword_line(
        #[case] type_line: &str,
        #[case] text: &str,
        #[case] kind: ClauseKind,
    ) {
        let compiled = one("Test Card", type_line, text);
        assert_eq!(compiled.clauses[0].kind, kind, "{text}");
    }

    #[test]
    fn a_manual_card_has_no_abilities_and_lists_its_clauses() {
        let compiled = one("Test Card", "Instant", "Flip a coin.\nFlip two coins.");
        let card = compiled.card.unwrap();
        assert_eq!(card.impl_status, ImplStatus::Manual);
        assert!(card.faces[0].abilities.is_empty(), "{card:?}");
        assert!(card.faces[0].oracle_fingerprint.is_empty(), "{card:?}");
        assert_eq!(compiled.clauses.len(), 2);
        assert!(
            compiled
                .clauses
                .iter()
                .all(|clause| { !clause.read && clause.kind == ClauseKind::Spell })
        );
    }

    #[test]
    fn a_text_with_only_reminder_text_has_no_card_and_no_clauses() {
        let compiled = one("Forest", "Basic Land \u{2014} Forest", "({T}: Add {G}.)");
        assert_eq!(compiled.card, None);
        assert!(compiled.clauses.is_empty(), "{compiled:?}");
    }

    #[test]
    fn every_face_of_a_double_faced_card_carries_its_own_fingerprint_and_name() {
        let front = "Haste\nAt the beginning of your upkeep, flip a coin.";
        let back = "Flying";
        let compiled = compile(&card(vec![
            face("Delver of Secrets", "Creature \u{2014} Human Wizard", front),
            face(
                "Insectile Aberration",
                "Creature \u{2014} Human Insect",
                back,
            ),
        ]));
        let card = compiled.card.unwrap();
        assert_eq!(card.name, "Delver of Secrets // Insectile Aberration");
        assert_eq!(card.impl_status, ImplStatus::Partial);
        assert_eq!(card.faces[0].oracle_fingerprint, oracle_fingerprint(front));
        assert_eq!(card.faces[1].oracle_fingerprint, oracle_fingerprint(back));
        assert_eq!(card.faces[1].abilities, [Ability::Keyword(Keyword::Flying)]);
        assert_eq!(
            compiled
                .clauses
                .iter()
                .map(|clause| clause.face)
                .collect::<Vec<_>>(),
            [0, 0, 1]
        );
        assert_eq!(
            compiled.clauses[1].text,
            "At the beginning of your upkeep, flip a coin."
        );
    }

    #[test]
    fn a_face_without_text_next_to_a_face_with_text_still_gets_a_fingerprint() {
        let compiled = compile(&card(vec![
            face("Front", "Creature \u{2014} Bear", ""),
            face("Back", "Creature \u{2014} Bear", "Flying"),
        ]));
        let card = compiled.card.unwrap();
        assert_eq!(card.impl_status, ImplStatus::Full);
        assert_eq!(card.faces[0].oracle_fingerprint, oracle_fingerprint(""));
        assert!(card.faces[0].abilities.is_empty(), "{card:?}");
    }

    #[rstest]
    #[case::full_name(
        "Atraxa, Praetors' Voice",
        "Legendary Creature \u{2014} Phyrexian",
        "Atraxa, Praetors' Voice has flying.",
        "~ has flying."
    )]
    #[case::short_name(
        "Atraxa, Praetors' Voice",
        "Legendary Creature \u{2014} Phyrexian",
        "Whenever Atraxa attacks, draw a card.",
        "Whenever ~ attacks, draw a card."
    )]
    #[case::not_legendary(
        "Fire, Ice",
        "Instant",
        "Fire deals 2 damage. Fire, Ice deals 1.",
        "Fire deals 2 damage. ~ deals 1."
    )]
    #[case::no_comma(
        "Questing Beast",
        "Legendary Creature \u{2014} Beast",
        "Questing Beast can't be blocked.",
        "~ can't be blocked."
    )]
    #[case::first_name_alone(
        "Legolas Greenleaf",
        "Legendary Creature \u{2014} Elf Archer",
        "Legolas can't be blocked. Whenever Legolas Greenleaf attacks, draw a card.",
        "~ can't be blocked. Whenever ~ attacks, draw a card."
    )]
    #[case::first_name_of_a_card_that_is_not_legendary(
        "Fire Imp",
        "Creature \u{2014} Imp",
        "Fire deals 2 damage.",
        "Fire deals 2 damage."
    )]
    #[case::an_article_is_no_name(
        "The Ur-Dragon",
        "Legendary Creature \u{2014} Dragon",
        "The Ur-Dragon flies. The best.",
        "~ flies. The best."
    )]
    fn the_card_names_become_tildes_in_the_clause_text(
        #[case] name: &str,
        #[case] type_line: &str,
        #[case] text: &str,
        #[case] expected: &str,
    ) {
        let compiled = one(name, type_line, text);
        assert_eq!(compiled.clauses[0].text, expected);
    }

    #[test]
    fn spans_stay_in_the_original_text_when_names_and_reminders_are_removed() {
        let text = "Haste (This creature can attack as soon as it comes under your control.)\nWhenever Atraxa attacks, draw a card. (It is nice.)\n\u{2212}X: Atraxa deals X damage.";
        let compiled = one(
            "Atraxa, Praetors' Voice",
            "Legendary Planeswalker \u{2014} Phyrexian",
            text,
        );
        let slices: Vec<_> = compiled
            .clauses
            .iter()
            .map(|clause| clause.span.slice(text).unwrap())
            .collect();
        assert_eq!(
            slices,
            [
                "Haste (This creature can attack as soon as it comes under your control.)",
                "Whenever Atraxa attacks, draw a card. (It is nice.)",
                "\u{2212}X: Atraxa deals X damage."
            ]
        );
        assert_eq!(compiled.clauses[2].kind, ClauseKind::Loyalty);
        assert_eq!(compiled.clauses[2].text, "\u{2212}X: ~ deals X damage.");
    }
}
