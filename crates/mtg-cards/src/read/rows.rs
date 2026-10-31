//! The rows of a sheet: the checker's pairing of clauses and abilities, each ability worded and walked.

use super::{ReadRow, ReadSheet, RowState, Step, steps};
use crate::ability::Ability;
use crate::card::{Card, Span};
use crate::compile::{ClauseKind, compile, render_ability, written_variants};
use crate::proposal::{ClauseReport, ClauseState, pair_card};
use crate::support::PrintedCard;

/// The ability a clause state names, when it names one.
const fn named(state: &ClauseState) -> Option<usize> {
    match state {
        ClauseState::Reads { ability }
        | ClauseState::Unwritten { ability, .. }
        | ClauseState::Differs { ability, .. } => Some(*ability),
        ClauseState::Manual { .. } | ClauseState::Open => None,
    }
}

/// Where each ability of a face stands: the clauses that name it, else the clause it was placed on.
struct Owners {
    /// For each ability, the index into the sheet's clauses of the first clause that shows it.
    head: Vec<Option<usize>>,
}

/// The indices of the clauses of a face, in the sheet's clause list.
fn clauses_of(clauses: &[ClauseReport], face: usize) -> Vec<usize> {
    (0..clauses.len())
        .filter(|&at| clauses[at].face == face)
        .collect()
}

/// Finds the clause that shows each ability of a face.
fn owners(
    clauses: &[ClauseReport],
    face: usize,
    abilities: &[Ability],
    places: &[Option<u32>],
) -> Owners {
    let of_face = clauses_of(clauses, face);
    let head = (0..abilities.len())
        .map(|ability| {
            of_face
                .iter()
                .copied()
                .find(|&at| named(&clauses[at].state) == Some(ability))
                .or_else(|| {
                    let start = places.get(ability).copied().flatten()?;
                    of_face
                        .iter()
                        .copied()
                        .find(|&at| clauses[at].span.start == start)
                })
        })
        .collect();
    Owners { head }
}

/// The wording of an ability: the renderer's first, else the first of the wordings the checker accepts.
pub(super) fn wording(ability: &Ability) -> Option<String> {
    render_ability(ability).or_else(|| written_variants(ability).into_iter().next())
}

/// A row's content for the abilities it shows.
fn content(shown: &[&Ability]) -> (String, Option<String>, Vec<Step>) {
    let label = shown
        .iter()
        .map(|ability| steps::label(ability))
        .collect::<Vec<_>>()
        .join(", ");
    let words: Vec<Option<String>> = shown.iter().map(|ability| wording(ability)).collect();
    let wording = if words.iter().all(Option::is_some) && !words.is_empty() {
        Some(words.into_iter().flatten().collect::<Vec<_>>().join(" "))
    } else {
        None
    };
    let list = shown
        .iter()
        .flat_map(|ability| steps::ability(ability))
        .collect();
    (label, wording, list)
}

/// The state a row shows.
fn state_of(state: &ClauseState) -> RowState {
    match state {
        ClauseState::Reads { .. } => RowState::Written,
        ClauseState::Unwritten { construct, .. } => RowState::Unwritten {
            construct: construct.clone(),
        },
        ClauseState::Differs { written, .. } => RowState::Differs {
            written: written.clone(),
        },
        ClauseState::Manual { reason } => RowState::ByHand {
            reason: Some(reason.clone()),
        },
        ClauseState::Open => RowState::ByHand { reason: None },
    }
}

/// The clause as printed: the stretch of the face's oracle text, else the clause as the compiler split it.
fn printed_words(printed: &PrintedCard, clause: &ClauseReport) -> String {
    printed
        .faces
        .get(clause.face)
        .and_then(|face| clause.span.slice(&face.oracle_text))
        .map_or_else(|| clause.text.clone(), str::to_owned)
}

/// The row of a clause that shows no ability.
fn by_hand_row(printed: &PrintedCard, clause: &ClauseReport) -> ReadRow {
    let steps = match &clause.state {
        ClauseState::Manual { reason } => vec![Step::new(0, format!("played by hand: {reason}"))],
        _ => vec![Step::new(
            0,
            "played by hand: the rules do not cover this clause",
        )],
    };
    ReadRow {
        face: clause.face,
        span: clause.span,
        kind: clause.kind,
        printed: printed_words(printed, clause),
        state: state_of(&clause.state),
        abilities: Vec::new(),
        continues: false,
        label: "played by hand".to_owned(),
        wording: None,
        steps,
    }
}

/// The row of an ability that no clause of the text stands behind.
fn extra_row(face: usize, index: usize, ability: &Ability) -> ReadRow {
    let (label, wording, steps) = content(&[ability]);
    ReadRow {
        face,
        span: Span { start: 0, end: 0 },
        kind: ClauseKind::Static,
        printed: String::new(),
        state: RowState::NoClause,
        abilities: vec![index],
        continues: false,
        label,
        wording,
        steps,
    }
}

/// Whether a `Manual` ability of a face leaves a clause to the table.
fn manual_is_placed(clauses: &[ClauseReport], face: usize, ability: &Ability) -> bool {
    let Ability::Manual { span, .. } = ability else {
        return false;
    };
    clauses
        .iter()
        .any(|clause| clause.face == face && clause.span.overlaps(span))
}

/// Builds the sheet of a card.
pub(super) fn build(card: &Card, printed: &PrintedCard) -> ReadSheet {
    let compiled = compile(printed).clauses;
    let paired = pair_card(card, &compiled, printed);
    let mut sheet = ReadSheet {
        card: printed.name.clone(),
        faces: printed.faces.iter().map(|face| face.name.clone()).collect(),
        rows: Vec::new(),
        extra: Vec::new(),
    };
    let faces = paired
        .clauses
        .iter()
        .map(|clause| clause.face + 1)
        .max()
        .unwrap_or(0)
        .max(card.faces.len());
    let mut shown_at: Vec<Vec<usize>> = vec![Vec::new(); paired.clauses.len()];
    for face in 0..faces {
        let abilities = card
            .faces
            .get(face)
            .map_or(&[][..], |rules| rules.abilities.as_slice());
        let places = paired.places.get(face).map_or(&[][..], Vec::as_slice);
        let found = owners(&paired.clauses, face, abilities, places);
        for (index, ability) in abilities.iter().enumerate() {
            match found.head[index] {
                Some(at) if !ability.is_manual() => shown_at[at].push(index),
                None if !ability.is_manual()
                    || !manual_is_placed(&paired.clauses, face, ability) =>
                {
                    sheet.extra.push(extra_row(face, index, ability));
                }
                _ => {}
            }
        }
    }
    let mut seen: Vec<(usize, usize)> = Vec::new();
    for (at, clause) in paired.clauses.iter().enumerate() {
        let rules = card
            .faces
            .get(clause.face)
            .map_or(&[][..], |rules| rules.abilities.as_slice());
        let named_ability = named(&clause.state);
        let continued = named_ability.is_some_and(|ability| seen.contains(&(clause.face, ability)));
        if let Some(ability) = named_ability {
            seen.push((clause.face, ability));
        }
        let row = if continued {
            continuation(printed, clause, named_ability)
        } else if shown_at[at].is_empty() {
            by_hand_row(printed, clause)
        } else {
            let shown: Vec<&Ability> = shown_at[at]
                .iter()
                .filter_map(|&index| rules.get(index))
                .collect();
            let (label, wording, list) = content(&shown);
            ReadRow {
                face: clause.face,
                span: clause.span,
                kind: clause.kind,
                printed: printed_words(printed, clause),
                state: state_of(&clause.state),
                abilities: shown_at[at].clone(),
                continues: false,
                label,
                wording,
                steps: list,
            }
        };
        sheet.rows.push(row);
    }
    sheet
}

/// The row of a later line of an ability that an earlier row shows.
fn continuation(printed: &PrintedCard, clause: &ClauseReport, ability: Option<usize>) -> ReadRow {
    ReadRow {
        face: clause.face,
        span: clause.span,
        kind: clause.kind,
        printed: printed_words(printed, clause),
        state: state_of(&clause.state),
        abilities: ability.into_iter().collect(),
        continues: true,
        label: "same ability as the clause above".to_owned(),
        wording: None,
        steps: vec![Step::new(0, "read together with the clause above")],
    }
}
